// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::array;
use std::collections::VecDeque;

use nv_redfish_core::ODataId;

use super::Lane;
use super::WorkRecord;
use super::LANE_COUNT;
use crate::BmcCapacity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TicketId(u64);

impl TicketId {
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug)]
pub(super) struct QueuedWork {
    pub(super) ticket_id: TicketId,
    pub(super) record: WorkRecord,
    owner: Option<ODataId>,
}

impl QueuedWork {
    pub(super) fn new(ticket_id: TicketId, record: WorkRecord) -> Self {
        let owner = (record.lane == Lane::Subscription).then(|| record.id.clone());
        Self {
            ticket_id,
            record,
            owner,
        }
    }

    pub(super) const fn lane(&self) -> Lane {
        self.record.lane
    }

    const fn owner(&self) -> Option<&ODataId> {
        self.owner.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LaneWeights {
    values: [u32; LANE_COUNT],
}

impl LaneWeights {
    #[cfg(test)]
    const fn from_values(values: [u32; LANE_COUNT]) -> Self {
        Self { values }
    }

    pub(super) const fn from_capacity(capacity: BmcCapacity) -> Self {
        Self {
            values: [
                capacity.lane_share(Lane::Interactive),
                capacity.lane_share(Lane::Subscription),
                capacity.lane_share(Lane::Discovery),
                capacity.lane_share(Lane::Maintenance),
            ],
        }
    }

    const fn get(self, lane: Lane) -> u32 {
        self.values[lane.index()]
    }
}

#[derive(Debug)]
pub(super) struct FairScheduler {
    weights: LaneWeights,
    queues: [VecDeque<QueuedWork>; LANE_COUNT],
    deficits: [u32; LANE_COUNT],
    next_lane: usize,
    selected: Option<TicketId>,
    last_subscription_owner: Option<ODataId>,
}

impl FairScheduler {
    pub(super) fn new(weights: LaneWeights) -> Self {
        Self {
            weights,
            queues: array::from_fn(|_| VecDeque::new()),
            deficits: [0; LANE_COUNT],
            next_lane: 0,
            selected: None,
            last_subscription_owner: None,
        }
    }

    pub(super) fn enqueue(&mut self, work: QueuedWork) {
        self.queues[work.lane().index()].push_back(work);
        self.clear_missing_selection();
    }

    pub(super) fn dispatch_if_selected(&mut self, ticket_id: TicketId) -> Option<QueuedWork> {
        if self.selected.is_none() {
            self.selected = self.select_next();
        }
        if self.selected != Some(ticket_id) {
            return None;
        }
        let work = self.remove(ticket_id)?;
        if work.lane() == Lane::Subscription {
            self.last_subscription_owner.clone_from(&work.owner);
        }
        self.selected = None;
        Some(work)
    }

    fn select_next(&mut self) -> Option<TicketId> {
        if self.is_empty() {
            return None;
        }
        loop {
            let index = self.next_lane;
            let lane = Lane::ALL[index];
            if self.queues[index].is_empty() {
                self.deficits[index] = 0;
                self.next_lane = (index + 1) % LANE_COUNT;
                continue;
            }
            if self.deficits[index] == 0 {
                self.deficits[index] = self.weights.get(lane);
            }
            if self.deficits[index] > 0 {
                self.deficits[index] -= 1;
                if self.deficits[index] == 0 {
                    self.next_lane = (index + 1) % LANE_COUNT;
                }
                return self.select_ticket(index);
            }
        }
    }

    fn select_ticket(&self, index: usize) -> Option<TicketId> {
        if Lane::ALL[index] != Lane::Subscription {
            return self.queues[index].front().map(|work| work.ticket_id);
        }
        let queue = &self.queues[index];
        let position = self
            .last_subscription_owner
            .as_ref()
            .and_then(|last_owner| {
                queue
                    .iter()
                    .position(|work| work.owner() != Some(last_owner))
            })
            .unwrap_or(0);
        queue.get(position).map(|work| work.ticket_id)
    }

    pub(super) fn remove(&mut self, ticket_id: TicketId) -> Option<QueuedWork> {
        for (index, queue) in self.queues.iter_mut().enumerate() {
            if let Some(position) = queue.iter().position(|work| work.ticket_id == ticket_id) {
                let removed = queue.remove(position);
                if queue.is_empty() {
                    self.deficits[index] = 0;
                    if self.next_lane == index {
                        self.next_lane = (self.next_lane + 1) % LANE_COUNT;
                    }
                }
                self.clear_missing_selection();
                return removed;
            }
        }
        self.clear_missing_selection();
        None
    }

    pub(super) fn queued(&self, lane: Lane) -> usize {
        self.queues[lane.index()].len()
    }

    fn is_empty(&self) -> bool {
        self.queues.iter().all(VecDeque::is_empty)
    }

    fn contains(&self, ticket_id: TicketId) -> bool {
        self.queues
            .iter()
            .any(|queue| queue.iter().any(|work| work.ticket_id == ticket_id))
    }

    fn clear_missing_selection(&mut self) {
        if self
            .selected
            .is_some_and(|ticket_id| !self.contains(ticket_id))
        {
            self.selected = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use nv_redfish_core::ODataId;

    use super::FairScheduler;
    use super::LaneWeights;
    use super::QueuedWork;
    use super::TicketId;
    use crate::scheduler::Lane;
    use crate::scheduler::Operation;
    use crate::scheduler::SchedulerState;
    use crate::scheduler::WorkRecord;
    use crate::BmcCapacity;

    fn dispatch_next(fair: &mut FairScheduler) -> Option<Lane> {
        let ticket_id = fair.select_next()?;
        fair.selected = Some(ticket_id);
        fair.dispatch_if_selected(ticket_id).map(|work| work.lane())
    }

    fn dispatch_next_id(fair: &mut FairScheduler) -> Option<ODataId> {
        let ticket_id = fair.select_next()?;
        fair.selected = Some(ticket_id);
        fair.dispatch_if_selected(ticket_id)
            .map(|work| work.record.id)
    }

    fn enqueue(fair: &mut FairScheduler, ticket: u64, lane: Lane, id: &str) {
        fair.enqueue(QueuedWork::new(TicketId(ticket), record(lane, id)));
    }

    fn enqueue_with_owner(
        fair: &mut FairScheduler,
        ticket: u64,
        lane: Lane,
        id: &str,
        owner: &str,
    ) {
        let mut work = QueuedWork::new(TicketId(ticket), record(lane, id));
        work.owner = Some(ODataId::from(owner.to_owned()));
        fair.enqueue(work);
    }

    fn record(lane: Lane, id: &str) -> WorkRecord {
        WorkRecord {
            lane,
            operation: Operation::Get,
            type_id: TypeId::of::<()>(),
            id: ODataId::from(id.to_owned()),
        }
    }

    fn fair(weights: [u32; 4]) -> FairScheduler {
        FairScheduler::new(LaneWeights::from_values(weights))
    }

    #[test]
    fn discovery_lane_makes_progress_under_subscription_load() {
        let mut fair = fair([1, 3, 1, 1]);
        for index in 0..12 {
            enqueue(
                &mut fair,
                index + 1,
                Lane::Subscription,
                &format!("/Subscription/{index}"),
            );
        }
        enqueue(&mut fair, 100, Lane::Discovery, "/Discovery/1");

        let dispatched = (0..4)
            .filter_map(|_| dispatch_next(&mut fair))
            .collect::<Vec<_>>();

        assert!(dispatched.contains(&Lane::Discovery));
    }

    #[test]
    fn interactive_lane_gets_service_before_background_work() {
        let mut fair = fair([5, 1, 1, 1]);
        enqueue(&mut fair, 1, Lane::Maintenance, "/Maintenance/1");
        enqueue(&mut fair, 2, Lane::Maintenance, "/Maintenance/2");
        enqueue(&mut fair, 3, Lane::Interactive, "/Interactive/1");

        assert_eq!(dispatch_next(&mut fair), Some(Lane::Interactive));
    }

    #[test]
    fn unused_lane_capacity_can_be_borrowed() {
        let mut fair = fair([5, 1, 1, 1]);
        for index in 0..5 {
            enqueue(
                &mut fair,
                index + 1,
                Lane::Subscription,
                &format!("/Subscription/{index}"),
            );
        }

        let dispatched = (0..5)
            .filter_map(|_| dispatch_next(&mut fair))
            .collect::<Vec<_>>();

        assert_eq!(dispatched, vec![Lane::Subscription; 5]);
    }

    #[test]
    fn borrowed_capacity_returns_when_lane_has_work() {
        let mut fair = fair([1, 3, 1, 1]);
        for index in 0..10 {
            enqueue(
                &mut fair,
                index + 1,
                Lane::Subscription,
                &format!("/Subscription/{index}"),
            );
        }
        assert_eq!(dispatch_next(&mut fair), Some(Lane::Subscription));
        assert_eq!(dispatch_next(&mut fair), Some(Lane::Subscription));
        enqueue(&mut fair, 100, Lane::Discovery, "/Discovery/1");

        let dispatched = [dispatch_next(&mut fair), dispatch_next(&mut fair)];

        assert!(dispatched.contains(&Some(Lane::Discovery)));
    }

    #[test]
    fn subscription_lane_rotates_between_owners() {
        let mut fair = fair([1, 4, 1, 1]);
        enqueue_with_owner(&mut fair, 1, Lane::Subscription, "/A/1", "owner-a");
        enqueue_with_owner(&mut fair, 2, Lane::Subscription, "/A/2", "owner-a");
        enqueue_with_owner(&mut fair, 3, Lane::Subscription, "/A/3", "owner-a");
        enqueue_with_owner(&mut fair, 4, Lane::Subscription, "/B/1", "owner-b");

        let first = dispatch_next_id(&mut fair);
        let second = dispatch_next_id(&mut fair);

        assert_eq!(first, Some(ODataId::from(String::from("/A/1"))));
        assert_eq!(second, Some(ODataId::from(String::from("/B/1"))));
    }

    #[test]
    fn scheduler_stats_expose_per_lane_queue_and_dispatch_counts() {
        let mut state = SchedulerState::new(BmcCapacity::fixed());
        let subscription = state.enqueue(record(Lane::Subscription, "/Subscription/1"));
        let _discovery = state.enqueue(record(Lane::Discovery, "/Discovery/1"));
        let queued = state.stats();

        assert_eq!(queued.queued, 2);
        assert_eq!(queued.subscription.queued, 1);
        assert_eq!(queued.discovery.queued, 1);

        let dispatched = state
            .dispatch_if_selected(subscription)
            .unwrap_or_else(|| state.stats());

        assert_eq!(dispatched.subscription.dispatched, 1);
        assert_eq!(dispatched.subscription.queued, 0);
        assert_eq!(dispatched.discovery.queued, 1);
    }
}
