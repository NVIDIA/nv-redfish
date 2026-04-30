// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use tokio::sync::Notify;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::time::sleep_until;
use tokio::time::Instant;

use crate::BmcCapacity;
use crate::Error;
use crate::EventBus;
use crate::SchedulerError;
use crate::ScraperEvent;

mod adaptive;
mod fair;

use adaptive::classify_error;
use adaptive::AdaptiveCapacity;
pub use adaptive::LoadState;
use adaptive::RequestOutcome;
use fair::FairScheduler;
use fair::LaneWeights;
use fair::QueuedWork;
use fair::TicketId;

/// Scheduler lane used for scraper work.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Lane {
    /// User-visible interactive work.
    Interactive,
    /// Long-lived subscription work.
    Subscription,
    /// Discovery work.
    Discovery,
    /// Background maintenance work.
    Maintenance,
}

impl Lane {
    const ALL: [Self; LANE_COUNT] = [
        Self::Interactive,
        Self::Subscription,
        Self::Discovery,
        Self::Maintenance,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Interactive => 0,
            Self::Subscription => 1,
            Self::Discovery => 2,
            Self::Maintenance => 3,
        }
    }
}

const LANE_COUNT: usize = 4;

/// Scheduler operation kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// Typed BMC `get` operation.
    Get,
}

/// Work item recorded by scheduler instrumentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRecord {
    /// Lane used for admission.
    pub lane: Lane,
    /// Operation kind.
    pub operation: Operation,
    /// Rust type id for the resource type.
    pub type_id: TypeId,
    /// Resource `@odata.id`.
    pub id: ODataId,
}

/// Scheduler event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerEvent {
    /// Scheduler load state changed.
    Stats {
        /// Current scheduler stats.
        state: SchedulerStats,
    },
    /// Adaptive BMC load state changed.
    LoadChanged {
        /// New load state.
        state: LoadState,
        /// Current adaptive in-flight limit.
        in_flight_limit: usize,
    },
}

/// Scheduler stats snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerStats {
    /// Number of requests currently admitted and not yet completed.
    pub in_flight: usize,
    /// Number of requests waiting for admission.
    pub queued: usize,
    /// Current adaptive in-flight request limit.
    pub in_flight_limit: usize,
    /// Current observed load state.
    pub load_state: LoadState,
    /// Interactive lane stats.
    pub interactive: LaneStats,
    /// Subscription lane stats.
    pub subscription: LaneStats,
    /// Discovery lane stats.
    pub discovery: LaneStats,
    /// Maintenance lane stats.
    pub maintenance: LaneStats,
}

/// Scheduler stats for one lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneStats {
    /// Requests waiting in this lane.
    pub queued: usize,
    /// Requests dispatched from this lane.
    pub dispatched: usize,
}

/// Fixed bounded scheduler for BMC requests.
///
/// This scheduler admits work through a fair lane gate, then applies hard
/// concurrency and request-rate limits. Adaptive behavior is added in a later
/// phase.
#[derive(Debug)]
pub struct Scheduler {
    capacity: BmcCapacity,
    in_flight: Arc<Semaphore>,
    state: Mutex<SchedulerState>,
    fair_ready: Notify,
    capacity_ready: Notify,
}

impl Scheduler {
    pub(crate) fn new(capacity: BmcCapacity) -> Self {
        Self {
            capacity,
            in_flight: Arc::new(Semaphore::new(capacity.max_in_flight_value())),
            state: Mutex::new(SchedulerState::new(capacity)),
            fair_ready: Notify::new(),
            capacity_ready: Notify::new(),
        }
    }

    pub(crate) async fn get<B, T>(
        &self,
        bmc: &B,
        events: &EventBus,
        lane: Lane,
        id: ODataId,
    ) -> Result<Arc<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let record = WorkRecord {
            lane,
            operation: Operation::Get,
            type_id: TypeId::of::<T>(),
            id: id.clone(),
        };
        let _admission = self.admit(record, events).await?;
        let started = Instant::now();
        let result = bmc.get::<T>(&id).await;
        let latency = started.elapsed();
        let outcome = result
            .as_ref()
            .map_or_else(|error| classify_error(error), |_| RequestOutcome::Success);
        self.record_observation(latency, outcome, events)?;
        result.map_err(Error::bmc)
    }

    async fn admit<'a>(
        &'a self,
        record: WorkRecord,
        events: &'a EventBus,
    ) -> Result<Admission<'a>, Error> {
        let queue_guard = self.record_queued(record, events)?;
        self.wait_for_fair_turn(queue_guard.ticket_id(), events)
            .await?;
        queue_guard.disarm();
        self.wait_for_capacity(events).await?;
        let permit = Arc::clone(&self.in_flight)
            .acquire_owned()
            .await
            .map_err(|error| {
                Error::Scheduler(SchedulerError::AdmissionClosed(error.to_string()))
            })?;
        self.wait_for_rate().await?;
        let in_flight_guard = self.record_admitted(events)?;
        Ok(Admission {
            _permit: permit,
            _guard: in_flight_guard,
        })
    }

    fn record_queued<'a>(
        &'a self,
        record: WorkRecord,
        events: &'a EventBus,
    ) -> Result<QueueGuard<'a>, Error> {
        let (ticket_id, stats) = {
            let mut state = self.state.lock().map_err(Error::scheduler_lock)?;
            let ticket_id = state.enqueue(record);
            (ticket_id, state.stats())
        };
        publish_stats(events, stats);
        self.fair_ready.notify_waiters();
        Ok(QueueGuard {
            state: &self.state,
            fair_ready: &self.fair_ready,
            events,
            ticket_id,
            active: true,
        })
    }

    async fn wait_for_fair_turn(
        &self,
        ticket_id: TicketId,
        events: &EventBus,
    ) -> Result<(), Error> {
        loop {
            let notified = self.fair_ready.notified();
            let dispatched = {
                let mut state = self.state.lock().map_err(Error::scheduler_lock)?;
                state.dispatch_if_selected(ticket_id).map(|stats| {
                    publish_stats(events, stats);
                })
            };
            if dispatched.is_some() {
                self.fair_ready.notify_waiters();
                return Ok(());
            }
            notified.await;
        }
    }

    async fn wait_for_capacity(&self, events: &EventBus) -> Result<(), Error> {
        loop {
            let notified = self.capacity_ready.notified();
            let admitted = {
                let mut state = self.state.lock().map_err(Error::scheduler_lock)?;
                if state.can_admit() {
                    state.record_adaptive_admitted();
                    Some(state.stats())
                } else {
                    None
                }
            };
            if let Some(stats) = admitted {
                publish_stats(events, stats);
                return Ok(());
            }
            notified.await;
        }
    }

    fn record_admitted<'a>(&'a self, events: &'a EventBus) -> Result<InFlightGuard<'a>, Error> {
        let stats = {
            let state = self.state.lock().map_err(Error::scheduler_lock)?;
            state.stats()
        };
        publish_stats(events, stats);
        Ok(InFlightGuard {
            state: &self.state,
            capacity_ready: &self.capacity_ready,
            events,
        })
    }

    fn record_observation(
        &self,
        latency: Duration,
        outcome: RequestOutcome,
        events: &EventBus,
    ) -> Result<(), Error> {
        let changed = {
            let mut state = self.state.lock().map_err(Error::scheduler_lock)?;
            state.observe(latency, outcome)
        };
        if let Some((stats, load_state, in_flight_limit)) = changed {
            publish_stats(events, stats);
            events.publish(ScraperEvent::Scheduler(SchedulerEvent::LoadChanged {
                state: load_state,
                in_flight_limit,
            }));
            self.capacity_ready.notify_waiters();
        }
        Ok(())
    }

    async fn wait_for_rate(&self) -> Result<(), Error> {
        let when = {
            let mut state = self.state.lock().map_err(Error::scheduler_lock)?;
            let now = Instant::now();
            let when = state.next_dispatch.map_or(now, |next| next.max(now));
            state.next_dispatch = Some(when + self.request_interval());
            when
        };
        sleep_until(when).await;
        Ok(())
    }

    fn request_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.capacity.max_requests_per_second_value()))
    }

    #[cfg(test)]
    pub(crate) async fn get_for_lane<B, T>(
        &self,
        bmc: &B,
        events: &EventBus,
        lane: Lane,
        id: ODataId,
    ) -> Result<Arc<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        self.get::<B, T>(bmc, events, lane, id).await
    }

    #[cfg(test)]
    pub(crate) fn records(&self) -> Result<Vec<WorkRecord>, Error> {
        self.state
            .lock()
            .map(|state| state.records.clone())
            .map_err(Error::scheduler_lock)
    }
}

fn publish_stats(events: &EventBus, state: SchedulerStats) {
    events.publish(ScraperEvent::Scheduler(SchedulerEvent::Stats { state }));
}

#[derive(Debug)]
struct Admission<'a> {
    _permit: OwnedSemaphorePermit,
    _guard: InFlightGuard<'a>,
}

#[derive(Debug)]
struct QueueGuard<'a> {
    state: &'a Mutex<SchedulerState>,
    fair_ready: &'a Notify,
    events: &'a EventBus,
    ticket_id: TicketId,
    active: bool,
}

impl QueueGuard<'_> {
    const fn ticket_id(&self) -> TicketId {
        self.ticket_id
    }

    fn disarm(mut self) {
        self.active = false;
    }
}

impl Drop for QueueGuard<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state.remove_queued(self.ticket_id);
            publish_stats(self.events, state.stats());
        }
        self.fair_ready.notify_waiters();
    }
}

#[derive(Debug)]
struct InFlightGuard<'a> {
    state: &'a Mutex<SchedulerState>,
    capacity_ready: &'a Notify,
    events: &'a EventBus,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.in_flight = state.in_flight.saturating_sub(1);
            publish_stats(self.events, state.stats());
        }
        self.capacity_ready.notify_waiters();
    }
}

#[derive(Debug)]
struct SchedulerState {
    records: Vec<WorkRecord>,
    queued: usize,
    in_flight: usize,
    next_dispatch: Option<Instant>,
    next_ticket_id: u64,
    fair: FairScheduler,
    adaptive: AdaptiveCapacity,
    dispatched_by_lane: [usize; LANE_COUNT],
}

impl SchedulerState {
    fn new(capacity: BmcCapacity) -> Self {
        Self {
            records: Vec::new(),
            queued: 0,
            in_flight: 0,
            next_dispatch: None,
            next_ticket_id: 1,
            fair: FairScheduler::new(LaneWeights::from_capacity(capacity)),
            adaptive: AdaptiveCapacity::new(capacity),
            dispatched_by_lane: [0; LANE_COUNT],
        }
    }

    fn enqueue(&mut self, record: WorkRecord) -> TicketId {
        let ticket_id = TicketId::new(self.next_ticket_id);
        self.next_ticket_id += 1;
        self.queued += 1;
        self.fair.enqueue(QueuedWork::new(ticket_id, record));
        ticket_id
    }

    fn dispatch_if_selected(&mut self, ticket_id: TicketId) -> Option<SchedulerStats> {
        let dispatched = self.fair.dispatch_if_selected(ticket_id)?;
        self.queued = self.queued.saturating_sub(1);
        self.dispatched_by_lane[dispatched.lane().index()] += 1;
        self.records.push(dispatched.record);
        Some(self.stats())
    }

    fn remove_queued(&mut self, ticket_id: TicketId) {
        if self.fair.remove(ticket_id).is_some() {
            self.queued = self.queued.saturating_sub(1);
        }
    }

    const fn can_admit(&self) -> bool {
        self.in_flight < self.adaptive.limit()
    }

    const fn record_adaptive_admitted(&mut self) {
        self.in_flight += 1;
    }

    fn observe(
        &mut self,
        latency: Duration,
        outcome: RequestOutcome,
    ) -> Option<(SchedulerStats, LoadState, usize)> {
        self.adaptive.observe(latency, outcome).then(|| {
            (
                self.stats(),
                self.adaptive.load_state(),
                self.adaptive.limit(),
            )
        })
    }

    fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            in_flight: self.in_flight,
            queued: self.queued,
            in_flight_limit: self.adaptive.limit(),
            load_state: self.adaptive.load_state(),
            interactive: self.lane_stats(Lane::Interactive),
            subscription: self.lane_stats(Lane::Subscription),
            discovery: self.lane_stats(Lane::Discovery),
            maintenance: self.lane_stats(Lane::Maintenance),
        }
    }

    fn lane_stats(&self, lane: Lane) -> LaneStats {
        LaneStats {
            queued: self.fair.queued(lane),
            dispatched: self.dispatched_by_lane[lane.index()],
        }
    }
}
