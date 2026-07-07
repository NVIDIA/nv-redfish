// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Generic dispatcher runtime.
//!
//! [`Runtime::next`] is the single ordered execution and output interface.
//! Each call drains the oldest queued output if one exists; otherwise it may
//! fill in-flight work up to [`RuntimeConfig::global_max_in_flight`], poll the
//! in-flight set, queue completed work outputs, and return once an output is
//! available. If no progress is possible, it parks.
//!
//! The runtime is policy-free and meta-blind. All scheduling lives in the
//! root [`Scheduler`] subtree; the runtime only dispatches payloads, polls
//! the in-flight set, forwards completions back through the tree, enforces
//! the runtime-wide caps in [`RuntimeConfig`], and emits [`RuntimeOutput`].
//!
//! The root sits behind an internal mutex. The driver and
//! [`RuntimeHandle::with_root`] / [`RuntimeHandle::with_root_mut`] share
//! the same lock, so user mutations and driver steps serialize naturally.

use crate::scheduler::private::SchedulerObj;
use crate::scheduler::Scheduler;
use crate::stats::OutputQueueStats;
use crate::stats::RuntimeStats;
use crate::work::WorkMeta;
use crate::Completion;
use crate::CompletionOutcome;
use crate::RoutingPath;
use crate::RuntimeEventType;
use crate::ScheduledWork;
use core::convert::TryFrom as _;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use core::task::Context;
use core::task::Poll;
use core::time::Duration;
use futures_core::Stream as _;
use futures_util::stream::FuturesUnordered;
use std::collections::VecDeque;
use std::mem;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;
use std::time::Instant;

/// `PhantomData` alias for the runtime's three type parameters; factored
/// to keep struct types simple under `clippy::type_complexity`.
type RuntimePhantom<Ev, Err, M> = PhantomData<fn() -> (Ev, Err, M)>;

/// Work payload consumed by this runtime: a boxed future with terminal
/// value `Result<Vec<Ev>, Err>`. Schedulers parameterized as
/// `Scheduler<FutureWork<Ev, Err>>` are compatible with [`Runtime::new`].
///
/// [`crate::Scheduler`] is generic over the payload and never inspects it,
/// so alternate runtimes can pick another shape (sync closures, batched
/// descriptors, …) and reuse the same scheduler types.
pub type FutureWork<Ev, Err> = Pin<Box<dyn Future<Output = Result<Vec<Ev>, Err>> + Send + 'static>>;

/// Generic dispatcher runtime, parameterized by event type `Ev`, error
/// type `Err`, and root meta type `M`.
///
/// `M` is whatever the root scheduler exposes as `Self::Meta` — typically
/// a stack of wrappers like `WithPriority<WithCost<()>>`.
///
/// Not `Clone`: only one consumer drives [`Runtime::next`]. Use
/// [`Runtime::handle`] for cloneable control handles.
pub struct Runtime<Ev, Err, M: WorkMeta> {
    config: RuntimeConfig,
    clock: RuntimeClock,
    in_flight: FuturesUnordered<InFlight<Ev, Err, M>>,
    completion: Vec<Completion<M>>,
    output: VecDeque<RuntimeOutput<Ev, Err>>,
    shared: Arc<Mutex<Shared<Ev, Err, M>>>,
    stats: Arc<StatsCells>,
    _phantom: RuntimePhantom<Ev, Err, M>,
}

impl<Ev, Err, M> Runtime<Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    /// Build a runtime with the given configuration and root scheduler.
    ///
    /// The bound `S: Scheduler<FutureWork<Ev, Err>, Meta = M>` ties the
    /// tree's payload to the shape this runtime executes. The root is
    /// consumed and stored behind a mutex; reach into it later with
    /// [`RuntimeHandle::with_root`] / [`RuntimeHandle::with_root_mut`] by
    /// supplying the same concrete type for the downcast.
    ///
    /// A blanket `impl Scheduler for Box<S>` lets you pass an existing
    /// `Box<dyn Scheduler<FutureWork<Ev, Err>, Meta = M>>` directly.
    #[must_use]
    pub fn new<S>(config: RuntimeConfig, root: S) -> Self
    where
        S: Scheduler<FutureWork<Ev, Err>, Meta = M>,
    {
        Self {
            clock: match config.clock {
                ClockConfig::Wallclock => RuntimeClock::Wallclock,
                ClockConfig::Virtual(increment) => RuntimeClock::Virtual {
                    now: Instant::now(),
                    increment,
                },
                ClockConfig::Manual(ref clock) => RuntimeClock::Manual(clock.clone()),
            },
            in_flight: FuturesUnordered::new(),
            completion: Vec::new(),
            output: VecDeque::new(),
            config,
            shared: Mutex::new(Shared {
                root: Box::new(root),
                waker: None,
                shutdown: false,
                _phantom: PhantomData,
            })
            .into(),
            stats: Arc::new(StatsCells::default()),
            _phantom: PhantomData,
        }
    }

    /// Cloneable handle for synchronous control and typed root access.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err, M> {
        RuntimeHandle {
            shared: self.shared.clone(),
            stats: self.stats.clone(),
        }
    }

    /// Handle to the manual clock; `None` unless configured with
    /// [`ClockConfig::Manual`].
    #[must_use]
    pub fn manual_clock(&self) -> Option<ManualClock> {
        match &self.clock {
            RuntimeClock::Manual(clock) => Some(clock.clone()),
            RuntimeClock::Wallclock | RuntimeClock::Virtual { .. } => None,
        }
    }

    /// Poll the in-flight set, converting finished payloads into pending
    /// completions and `Work` outputs. Returns `true` if anything
    /// finished.
    fn drain_completed(
        &mut self,
        in_flight: &mut FuturesUnordered<InFlight<Ev, Err, M>>,
        cx: &mut Context<'_>,
        now: Instant,
    ) -> bool {
        let mut progress = false;
        while let Poll::Ready(Some(completed)) = Pin::new(&mut *in_flight).poll_next(cx) {
            let CompletedWork {
                start,
                meta,
                result,
                routing,
            } = completed;
            let latency = now.duration_since(start);
            progress = true;
            self.completion.push(Completion {
                latency,
                meta,
                routing,
                outcome: if result.is_ok() {
                    CompletionOutcome::Succeeded
                } else {
                    CompletionOutcome::Failed
                },
            });
            self.output
                .push_back(RuntimeOutput::Work { result, latency });
        }
        progress
    }

    /// Advance until an output is available or no progress is possible.
    ///
    /// Step order:
    ///
    /// 1. Drain one queued output if any.
    /// 2. If below [`RuntimeConfig::global_max_in_flight`], lock shared
    ///    scheduler/control state briefly and dispatch available work until
    ///    the global cap is reached or the scheduler has no work.
    /// 3. If shared state has no work while capacity remains, register the
    ///    current waker so external control changes can wake this future.
    /// 4. Poll in-flight payloads without holding the shared lock; completed
    ///    payloads enqueue [`RuntimeOutput::Work`].
    /// 5. If no output was queued and no synchronous progress was made, park.
    ///
    /// Shares the root lock with [`RuntimeHandle::with_root_mut`]; both
    /// hold it briefly.
    pub const fn next(&mut self) -> NextFuture<'_, Ev, Err, M> {
        NextFuture { runtime: self }
    }
}

/// Future returned by [`Runtime::next`].
pub struct NextFuture<'r, Ev, Err, M: WorkMeta> {
    // Exclusive borrow enforces the single-driver invariant.
    runtime: &'r mut Runtime<Ev, Err, M>,
}

impl<Ev, Err, M> Future for NextFuture<'_, Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    type Output = RuntimeOutput<Ev, Err>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut progress = true;
        while progress {
            if !self.runtime.completion.is_empty() {
                let completions = mem::take(&mut self.runtime.completion);
                let mut shared = self
                    .runtime
                    .shared
                    .lock()
                    .expect("dispatcher runtime mutex is poisoned");
                for completion in completions {
                    shared.root.on_complete(completion);
                }
            }
            if let Some(output) = self.runtime.output.pop_front() {
                self.runtime
                    .stats
                    .output_queued
                    .store(self.runtime.output.len(), Ordering::Relaxed);
                return Poll::Ready(output);
            }
            progress = false;

            let now = self.runtime.clock.now();
            let mut in_flight = mem::take(&mut self.runtime.in_flight);
            let global_max_in_flight = self.runtime.config.global_max_in_flight;
            let mut shutdown = false;
            if in_flight.len() < global_max_in_flight.into() {
                let mut shared = self
                    .runtime
                    .shared
                    .lock()
                    .expect("dispatcher runtime mutex is poisoned");
                shutdown = shared.shutdown;
                while !shutdown && in_flight.len() < global_max_in_flight.into() {
                    match shared.next(now) {
                        SharedNextResult::Work(work) => {
                            self.runtime
                                .stats
                                .dispatched
                                .fetch_add(1, Ordering::Relaxed);
                            in_flight.push(InFlight {
                                start: now,
                                work: Some(work),
                            });
                        }
                        SharedNextResult::SleepUntil(v) => {
                            if in_flight.is_empty() {
                                return Poll::Ready(RuntimeOutput::SleepUntil(v));
                            }
                            break;
                        }
                        SharedNextResult::Nothing => {
                            break;
                        }
                    }
                }
                // Setup waker if change in shared elements can cause
                // progress for the future.
                if in_flight.len() < global_max_in_flight.into() {
                    shared.maybe_setup_waker(cx);
                }
            }
            self.runtime
                .stats
                .in_flight
                .store(in_flight.len() as u64, Ordering::Relaxed);

            progress |= self.runtime.drain_completed(&mut in_flight, cx, now);
            self.runtime
                .stats
                .in_flight
                .store(in_flight.len() as u64, Ordering::Relaxed);
            self.runtime
                .stats
                .output_queued
                .store(self.runtime.output.len(), Ordering::Relaxed);
            self.runtime.in_flight = in_flight;

            // Shutdown is emitted only once everything has drained.
            if shutdown
                && self.runtime.in_flight.is_empty()
                && self.runtime.output.is_empty()
                && self.runtime.completion.is_empty()
            {
                return Poll::Ready(RuntimeOutput::Shutdown);
            }
        }
        Poll::Pending
    }
}

/// Runtime-wide configuration. Per-node policy lives inside each
/// [`Scheduler`]; this struct only carries knobs no node owns.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Global cap on in-flight items, applied at dispatch admission
    /// on top of any per-subtree admission a branch enforces.
    pub global_max_in_flight: NonZeroUsize,
    /// Runtime clock configuration.
    pub clock: ClockConfig,
}

/// Runtime clock configuration.
#[derive(Clone, Debug, Default)]
pub enum ClockConfig {
    /// Clock that ticks with real time.
    #[default]
    Wallclock,
    /// Clock that increments on specified duration each time it is
    /// requested.
    Virtual(Duration),
    /// Clock driven by the given [`ManualClock`] handle. Construct the
    /// clock first and build time-holding scheduler nodes with
    /// `clock.now()`, so the tree and the runtime share one epoch.
    Manual(ManualClock),
}

/// Manually advanced clock for [`ClockConfig::Manual`] runtimes.
///
/// Cloneable; time starts at the construction instant and only moves
/// forward via [`ManualClock::advance`] / [`ManualClock::advance_to`].
/// Advancing does not wake the runtime — call [`Runtime::next`] afterwards.
#[derive(Clone, Debug)]
pub struct ManualClock {
    epoch: Instant,
    offset_nanos: Arc<AtomicU64>,
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ManualClock {
    /// Clock whose epoch is the construction instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
            offset_nanos: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Current virtual time.
    #[must_use]
    pub fn now(&self) -> Instant {
        self.epoch + Duration::from_nanos(self.offset_nanos.load(Ordering::Relaxed))
    }

    /// Move time forward by `by`.
    pub fn advance(&self, by: Duration) {
        let by = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
        let _ = self
            .offset_nanos
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some(cur.saturating_add(by))
            });
    }

    /// Move time forward to `to`. Targets in the past are ignored: the
    /// clock is monotonic.
    pub fn advance_to(&self, to: Instant) {
        let target =
            u64::try_from(to.saturating_duration_since(self.epoch).as_nanos()).unwrap_or(u64::MAX);
        self.offset_nanos.fetch_max(target, Ordering::Relaxed);
    }
}

/// Cloneable handle to a running [`Runtime`].
///
/// Exposes synchronous control plus typed root access. Mutating ops take
/// the internal lock briefly and never wait on work payloads. The runtime
/// itself is not `Clone`.
pub struct RuntimeHandle<Ev, Err, M: WorkMeta> {
    shared: Arc<Mutex<Shared<Ev, Err, M>>>,
    stats: Arc<StatsCells>,
}

impl<Ev, Err, M: WorkMeta> Clone for RuntimeHandle<Ev, Err, M> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            stats: self.stats.clone(),
        }
    }
}

impl<Ev, Err, M> RuntimeHandle<Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    /// Begin graceful shutdown. Idempotent. In-flight items still complete,
    /// queued outputs still drain, then [`Runtime::next`] emits a sticky
    /// shutdown.
    ///
    /// Wakes a driver parked inside [`Runtime::next`]; a driver sleeping
    /// on a [`RuntimeOutput::SleepUntil`] hint observes the shutdown at
    /// its next `next()` call.
    ///
    /// # Panics
    ///
    /// Can panic if the runtime mutex is poisoned, which only happens if
    /// a closure passed to [`Self::with_root_mut`] panicked.
    pub fn graceful_shutdown(&self) {
        let mut guard = self
            .shared
            .lock()
            .expect("dispatcher runtime mutex is poisoned");
        guard.shutdown = true;
        let waker = guard.waker.take();
        drop(guard);
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Snapshot of runtime statistics. Lock-free; values are relaxed
    /// snapshots and may trail the driver by one step.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats {
            in_flight: self.stats.in_flight.load(Ordering::Relaxed),
            dispatched: self.stats.dispatched.load(Ordering::Relaxed),
            output_queue: OutputQueueStats {
                queued: self.stats.output_queued.load(Ordering::Relaxed),
                capacity: None,
                dropped: 0,
            },
        }
    }

    /// Run `f` with shared access to the root downcast to `S`. `None` if
    /// the downcast fails.
    ///
    /// Holds the root lock for the duration of `f`; keep it short and do
    /// not re-enter the runtime from inside (it will deadlock).
    ///
    /// # Panics
    ///
    /// Can panic if the runtime mutex is poisoned, which only happens if
    /// a closure passed to [`Self::with_root_mut`] panicked.
    #[allow(clippy::unwrap_in_result)]
    pub fn with_root<S, R>(&self, f: impl FnOnce(&S) -> R) -> Option<R>
    where
        S: 'static,
    {
        let guard = self
            .shared
            .lock()
            .expect("dispatcher runtime mutex is poisoned");
        guard.root.as_any().downcast_ref::<S>().map(f)
    }

    /// Run `f` with exclusive access to the root downcast to `S`. `None`
    /// if the downcast fails.
    ///
    /// Holds the root lock for the duration of `f`; keep it short and do
    /// not re-enter the runtime from inside (it will deadlock).
    ///
    /// # Panics
    ///
    /// Can panic if runtime mutex is poisoned. Which only can happen
    /// if any f passed to this function paniced.
    #[allow(clippy::unwrap_in_result)]
    pub fn with_root_mut<S, R>(&self, f: impl FnOnce(&mut S) -> R) -> Option<R>
    where
        S: 'static,
    {
        let mut guard = self
            .shared
            .lock()
            .expect("dispatcher runtime mutex is poisoned");
        let result = guard.root.as_any_mut().downcast_mut::<S>().map(f);
        let waker = guard.waker.take();
        drop(guard);
        if let Some(waker) = waker {
            waker.wake();
        }
        result
    }
}

/// Single ordered output emitted by the runtime.
///
/// `R` defaults to [`crate::RuntimeEventType`], which is
/// [`core::convert::Infallible`] when the `runtime-events` feature is off
/// — `RuntimeOutput::Runtime(_)` is then unconstructible.
pub enum RuntimeOutput<Ev, Err, R = RuntimeEventType> {
    /// Terminal value of one work payload, plus its wall-clock latency.
    Work {
        /// `Ok(events)` (one or more events in order) or `Err(error)`.
        result: Result<Vec<Ev>, Err>,
        /// Latency between dispatch and completion.
        latency: Duration,
    },
    /// Out-of-band runtime event (only when `runtime-events` is enabled).
    Runtime(R),
    /// Runtime requested to sleep until the given instant before the
    /// next `next()` call (e.g. `tokio::time::sleep`); calling earlier
    /// is always safe.
    ///
    /// Control-plane changes ([`RuntimeHandle::graceful_shutdown`],
    /// [`RuntimeHandle::with_root_mut`]) are only observed at the next
    /// `next()` call. Drivers needing a prompt reaction should race the
    /// sleep against their own wake signal (e.g. `tokio::select!`).
    SleepUntil(Instant),
    /// Sticky terminal output after graceful shutdown drains. Subsequent
    /// `next()` calls return this immediately.
    Shutdown,
}

enum RuntimeClock {
    Wallclock,
    Virtual { now: Instant, increment: Duration },
    Manual(ManualClock),
}

impl RuntimeClock {
    fn now(&mut self) -> Instant {
        match self {
            Self::Wallclock => Instant::now(),
            Self::Virtual { now, increment } => {
                *now += *increment;
                *now
            }
            Self::Manual(clock) => clock.now(),
        }
    }
}

/// Runtime-wide counters shared lock-free between the driver and
/// [`RuntimeHandle::stats`].
#[derive(Default)]
struct StatsCells {
    dispatched: AtomicU64,
    in_flight: AtomicU64,
    output_queued: AtomicUsize,
}

struct Shared<Ev, Err, M> {
    waker: Option<Waker>,
    root: Box<dyn SchedulerObj<FutureWork<Ev, Err>, M>>,
    shutdown: bool,
    _phantom: PhantomData<(Ev, Err)>,
}

impl<Ev, Err, M> Shared<Ev, Err, M>
where
    Ev: 'static,
    Err: 'static,
    M: Send + 'static,
{
    fn next(&mut self, now: Instant) -> SharedNextResult<Ev, Err, M> {
        let r = self.root.update_ready(now);
        if r.ready {
            if let Some(work) = self.root.take_next() {
                return SharedNextResult::Work(work);
            }
        }
        r.next_update_at
            .map_or(SharedNextResult::Nothing, SharedNextResult::SleepUntil)
    }

    fn maybe_setup_waker(&mut self, cx: &Context<'_>) {
        let waker = cx.waker();

        if self
            .waker
            .as_ref()
            .is_none_or(|old_waker| !old_waker.will_wake(waker))
        {
            self.waker = Some(waker.clone());
        }
    }
}

enum SharedNextResult<Ev, Err, M>
where
    Ev: 'static,
    Err: 'static,
    M: Send + 'static,
{
    Work(ScheduledWork<FutureWork<Ev, Err>, M>),
    SleepUntil(Instant),
    Nothing,
}

struct InFlight<Ev, Err, M: WorkMeta> {
    start: Instant,
    work: Option<ScheduledWork<FutureWork<Ev, Err>, M>>,
}

impl<Ev, Err, M: WorkMeta> Unpin for InFlight<Ev, Err, M> {}

impl<Ev, Err, M: WorkMeta> Future for InFlight<Ev, Err, M> {
    type Output = CompletedWork<Ev, Err, M>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut work = this
            .work
            .take()
            .expect("in-flight work polled after completion");
        match work.payload.as_mut().poll(cx) {
            Poll::Pending => {
                this.work = Some(work);
                Poll::Pending
            }
            Poll::Ready(result) => Poll::Ready(CompletedWork {
                start: this.start,
                result,
                meta: work.meta,
                routing: work.routing,
            }),
        }
    }
}

struct CompletedWork<Ev, Err, M> {
    start: Instant,
    meta: M,
    result: Result<Vec<Ev>, Err>,
    routing: RoutingPath,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use core::time::Duration;
    use std::{num::NonZeroUsize, time::Instant};

    use futures_util::future::poll_immediate;

    use super::{ClockConfig, FutureWork, Runtime, RuntimeConfig, RuntimeOutput};
    use crate::schedulers::{PeriodicLeaf, RoundRobin};

    type TestWork = FutureWork<u64, String>;
    type TestRoot = RoundRobin<TestWork, ()>;

    fn config() -> RuntimeConfig {
        RuntimeConfig {
            global_max_in_flight: NonZeroUsize::new(2).expect("non-zero"),
            clock: ClockConfig::Wallclock,
        }
    }

    fn firing_root() -> TestRoot {
        let mut root = TestRoot::new();
        root.add_child(PeriodicLeaf::new(Instant::now(), Duration::ZERO, || {
            Box::pin(async { Ok(vec![7_u64]) }) as TestWork
        }));
        root
    }

    #[tokio::test]
    async fn graceful_shutdown_drains_then_is_sticky() {
        let mut rt: Runtime<u64, String, ()> = Runtime::new(config(), firing_root());
        let handle = rt.handle();

        let mut works = 0_u64;
        while works < 3 {
            if let RuntimeOutput::Work { result, .. } = rt.next().await {
                assert_eq!(result.expect("payload succeeds"), vec![7]);
                works += 1;
            }
        }

        handle.graceful_shutdown();
        // Anything already in flight still drains as Work outputs, then
        // the terminal Shutdown arrives.
        loop {
            match rt.next().await {
                RuntimeOutput::Work { .. } => works += 1,
                RuntimeOutput::Shutdown => break,
                RuntimeOutput::SleepUntil(_) | RuntimeOutput::Runtime(_) => {}
            }
        }
        // Sticky: every subsequent call returns Shutdown immediately.
        assert!(matches!(rt.next().await, RuntimeOutput::Shutdown));
        assert!(matches!(rt.next().await, RuntimeOutput::Shutdown));

        let stats = handle.stats();
        assert_eq!(stats.dispatched, works, "every dispatch was drained");
        assert_eq!(stats.in_flight, 0);
        assert_eq!(stats.output_queue.queued, 0);
    }

    #[tokio::test]
    async fn graceful_shutdown_wakes_a_parked_driver() {
        // An empty root reports not-ready with no hint: the driver parks.
        let mut rt: Runtime<u64, String, ()> = Runtime::new(config(), TestRoot::new());
        let handle = rt.handle();

        let mut fut = rt.next();
        assert!(
            poll_immediate(&mut fut).await.is_none(),
            "nothing to do: the driver must park"
        );
        handle.graceful_shutdown();
        assert!(matches!(fut.await, RuntimeOutput::Shutdown));
    }

    #[tokio::test]
    async fn stats_count_dispatches() {
        let mut rt: Runtime<u64, String, ()> = Runtime::new(config(), firing_root());
        let handle = rt.handle();
        assert_eq!(handle.stats().dispatched, 0);

        let mut works = 0_u64;
        while works < 5 {
            if let RuntimeOutput::Work { .. } = rt.next().await {
                works += 1;
            }
        }
        assert!(handle.stats().dispatched >= 5);
    }

    #[tokio::test]
    async fn with_root_reads_and_with_root_mut_mutates() {
        let rt: Runtime<u64, String, ()> = Runtime::new(config(), firing_root());
        let handle = rt.handle();

        assert_eq!(handle.with_root::<TestRoot, _>(TestRoot::len), Some(1));
        assert_eq!(handle.with_root::<u32, _>(|_| ()), None, "wrong type");

        let id = handle
            .with_root_mut::<TestRoot, _>(|root| {
                root.add_child(PeriodicLeaf::new(
                    Instant::now(),
                    Duration::from_secs(9999),
                    || Box::pin(async { Ok(vec![9_u64]) }) as TestWork,
                ))
            })
            .expect("root downcasts");
        assert_eq!(handle.with_root::<TestRoot, _>(TestRoot::len), Some(2));

        handle
            .with_root_mut::<TestRoot, _>(|root| {
                root.remove_child(id).expect("child exists");
            })
            .expect("root downcasts");
        assert_eq!(handle.with_root::<TestRoot, _>(TestRoot::len), Some(1));
    }
}
