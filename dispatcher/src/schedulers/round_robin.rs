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

//! Plain round-robin branch with dynamic membership.
//!
//! Children are stored in a map keyed by a `u32` id (the routing tag).
//! Iteration order is a `VecDeque<u32>` of ids: `take_next` pops the
//! front, asks that child, pushes the id back, and stops on the first
//! `Some`. Children appended mid-scan are visited within one cycle.
//!
//! Children can be removed at any time. A child with nothing in flight
//! is handed back ([`RemovedChild::Detached`]); one with items
//! outstanding is quarantined out of rotation, keeps receiving its
//! completions, and is dropped once drained ([`RemovedChild::Draining`]).
//! Ids are recycled only after the drain, so id consumption is bounded
//! by concurrent children and a late completion is never misrouted to a
//! child that inherited the id.

use core::marker::PhantomData;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::scheduler::{ScheduledWork, Scheduler};
use crate::work::{Completion, Readiness, WorkMeta};

/// A live child plus the number of its items currently in flight.
struct Slot<T, M: WorkMeta> {
    sched: Box<dyn Scheduler<T, Meta = M>>,
    in_flight: u32,
}

/// Outcome of [`RoundRobin::remove_child`].
pub enum RemovedChild<T, M: WorkMeta> {
    /// Nothing was in flight: the subtree is handed back fully drained
    /// and safe to reuse.
    Detached(Box<dyn Scheduler<T, Meta = M>>),
    /// Items were still in flight: the subtree stays quarantined inside
    /// the branch, receives its remaining completions, and is dropped
    /// once drained.
    Draining,
}

/// Round robing over boxed children
pub struct RoundRobin<T, M: WorkMeta> {
    children: HashMap<u32, Slot<T, M>>,
    /// Removed children still owed completions: out of rotation, but
    /// completions are forwarded until `in_flight` drains to zero, then
    /// the subtree is dropped and the id moves to `free`.
    draining: HashMap<u32, Slot<T, M>>,
    /// Ids safe to hand out again.
    free: Vec<u32>,
    queue: VecDeque<u32>,
    next_id: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T, M: WorkMeta> Default for RoundRobin<T, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, M: WorkMeta> RoundRobin<T, M> {
    /// Empty round-robin branch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            draining: HashMap::new(),
            free: Vec::new(),
            queue: VecDeque::new(),
            next_id: 0,
            _t: PhantomData,
        }
    }

    /// Append `child` and return its id (the routing tag). Ids of
    /// removed children are recycled: a cached id is invalidated by
    /// [`Self::remove_child`] and may later address a different child —
    /// do not hold ids across a removal you don't control.
    ///
    /// # Panics
    ///
    /// Panics if more than `u32::MAX` children are held *concurrently*
    /// (live plus removed-but-draining), which the
    /// [`RoutingPath`](crate::RoutingPath) tag width does not support.
    pub fn add_child<S>(&mut self, child: S) -> u32
    where
        S: Scheduler<T, Meta = M>,
    {
        let id = if let Some(id) = self.free.pop() {
            id
        } else {
            let id = self.next_id;
            self.next_id = self
                .next_id
                .checked_add(1)
                .expect("RoundRobin supports up to u32::MAX concurrent children");
            id
        };
        self.children.insert(
            id,
            Slot {
                sched: Box::new(child),
                in_flight: 0,
            },
        );
        self.queue.push_back(id);
        id
    }

    /// Remove the child with the given id, or `None` if no such child
    /// exists.
    ///
    /// With nothing in flight the subtree is returned
    /// ([`RemovedChild::Detached`]); otherwise it stays quarantined —
    /// out of rotation, still receiving its remaining completions — and
    /// is dropped once drained ([`RemovedChild::Draining`]). The id is
    /// reusable by [`Self::add_child`] only after the drain finishes.
    pub fn remove_child(&mut self, id: u32) -> Option<RemovedChild<T, M>> {
        let slot = self.children.remove(&id)?;
        // Purge eagerly: a recycled id must not still sit in the queue
        // when add_child pushes it again.
        self.queue.retain(|&queued| queued != id);
        if slot.in_flight > 0 {
            self.draining.insert(id, slot);
            Some(RemovedChild::Draining)
        } else {
            self.free.push(id);
            Some(RemovedChild::Detached(slot.sched))
        }
    }

    /// Number of children currently held by this branch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// `true` when the branch currently holds no children.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl<T, M> Scheduler<T> for RoundRobin<T, M>
where
    T: Send + 'static,
    M: WorkMeta,
{
    type Meta = M;

    fn update_ready(&mut self, now: Instant) -> Readiness {
        let mut ready = false;
        let mut next_at: Option<Instant> = None;
        for slot in self.children.values_mut() {
            let r = slot.sched.update_ready(now);
            ready |= r.ready;
            next_at = match (next_at, r.next_update_at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
        Readiness {
            ready,
            next_update_at: next_at,
            next_cost: None,
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<T, M>> {
        let n = self.queue.len();
        for _ in 0..n {
            let id = self.queue.pop_front()?;
            let Some(slot) = self.children.get_mut(&id) else {
                // Defensive: remove_child purges the queue eagerly.
                continue;
            };
            self.queue.push_back(id);
            if let Some(mut work) = slot.sched.take_next() {
                slot.in_flight = slot.in_flight.saturating_add(1);
                work.routing.push(id);
                return Some(work);
            }
        }
        None
    }

    fn on_complete(&mut self, mut completion: Completion<M>) {
        let Some(id) = completion.routing.pop() else {
            return;
        };
        if let Some(slot) = self.children.get_mut(&id) {
            slot.in_flight = slot.in_flight.saturating_sub(1);
            slot.sched.on_complete(completion);
        } else if let Some(slot) = self.draining.get_mut(&id) {
            // Forward to the quarantined child; recycle the id and drop
            // the subtree once drained.
            slot.in_flight = slot.in_flight.saturating_sub(1);
            slot.sched.on_complete(completion);
            if slot.in_flight == 0 {
                self.draining.remove(&id);
                self.free.push(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use core::time::Duration;
    use std::time::Instant;

    use super::{RemovedChild, RoundRobin};
    use crate::scheduler::Scheduler as _;
    use crate::schedulers::tests::{dispatch_and_complete, MockLeaf};
    use crate::work::{Completion, CompletionOutcome};

    #[test]
    fn empty_branch_is_not_ready_and_yields_nothing() {
        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let now = Instant::now();
        let r = rr.update_ready(now);
        assert!(!r.ready);
        assert!(rr.take_next().is_none());
    }

    #[test]
    fn single_child_round_trip() {
        let leaf = MockLeaf::ready_firing(11);
        let handle = leaf.handle();
        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let id = rr.add_child(leaf);
        assert_eq!(id, 0);

        let now = Instant::now();
        assert!(rr.update_ready(now).ready);

        let routing = dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
            .expect("work should be dispatched");
        // The routing path observed at the parent already contains the
        // child id stamped by RR.
        assert_eq!(routing.depth(), 1);
        assert_eq!(handle.completion_count(), 1);
    }

    #[test]
    fn three_children_rotate_evenly() {
        let l0 = MockLeaf::ready_firing(100);
        let l1 = MockLeaf::ready_firing(200);
        let l2 = MockLeaf::ready_firing(300);
        let h0 = l0.handle();
        let h1 = l1.handle();
        let h2 = l2.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        rr.add_child(l0);
        rr.add_child(l1);
        rr.add_child(l2);

        for _ in 0..6 {
            dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
                .expect("ready leaf should produce work");
        }

        assert_eq!(h0.completion_count(), 2);
        assert_eq!(h1.completion_count(), 2);
        assert_eq!(h2.completion_count(), 2);
    }

    #[test]
    fn skips_not_ready_children() {
        let l0 = MockLeaf::ready_firing(1);
        let l1 = MockLeaf::ready_idle();
        let l2 = MockLeaf::ready_firing(3);
        let h0 = l0.handle();
        let h1 = l1.handle();
        let h2 = l2.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        rr.add_child(l0);
        rr.add_child(l1);
        rr.add_child(l2);

        for _ in 0..4 {
            dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
                .expect("at least one leaf is firing");
        }

        // l1 never fires, the other two split the work.
        assert_eq!(h0.completion_count() + h2.completion_count(), 4);
        assert_eq!(h1.completion_count(), 0);
    }

    #[test]
    fn add_child_mid_scan_is_visited_within_one_cycle() {
        let l0 = MockLeaf::ready_firing(1);
        let l1 = MockLeaf::ready_firing(2);
        let h0 = l0.handle();
        let h1 = l1.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        rr.add_child(l0);
        rr.add_child(l1);

        // Pull one item to advance the cursor.
        dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO).expect("ok");

        // Add a third child mid-stream.
        let l2 = MockLeaf::ready_firing(3);
        let h2 = l2.handle();
        rr.add_child(l2);

        // Pull three items: every child must be visited at least once.
        for _ in 0..3 {
            dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
                .expect("ok");
        }

        assert!(h0.completion_count() >= 1);
        assert!(h1.completion_count() >= 1);
        assert!(h2.completion_count() >= 1);
    }

    #[test]
    fn removed_child_is_no_longer_scheduled() {
        let l0 = MockLeaf::ready_firing(1);
        let l1 = MockLeaf::ready_firing(2);
        let h0 = l0.handle();
        let h1 = l1.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let id0 = rr.add_child(l0);
        rr.add_child(l1);
        assert_eq!(rr.len(), 2);

        assert!(rr.remove_child(id0).is_some());
        assert!(rr.remove_child(id0).is_none(), "second removal is a miss");
        assert_eq!(rr.len(), 1);

        for _ in 0..4 {
            dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
                .expect("remaining leaf fires");
        }
        assert_eq!(h0.completion_count(), 0);
        assert_eq!(h1.completion_count(), 4);
    }

    #[test]
    fn in_flight_removal_quarantines_and_forwards_the_late_completion() {
        let l0 = MockLeaf::ready_firing(1);
        let h0 = l0.handle();
        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let id0 = rr.add_child(l0);

        // Dispatch, then remove the child while the item is in flight:
        // the subtree is quarantined, not handed back.
        let work = rr.take_next().expect("leaf fires");
        assert!(matches!(rr.remove_child(id0), Some(RemovedChild::Draining)));

        // A replacement child must get a fresh id while the old id is
        // owed a completion, so the completion cannot alias onto it.
        let l1 = MockLeaf::ready_firing(2);
        let h1 = l1.handle();
        let id1 = rr.add_child(l1);
        assert_ne!(id0, id1);

        rr.on_complete(Completion {
            outcome: CompletionOutcome::Succeeded,
            latency: Duration::ZERO,
            meta: work.meta,
            routing: work.routing,
        });
        assert_eq!(h1.completion_count(), 0, "not misrouted to the new child");
        assert_eq!(
            h0.completion_count(),
            1,
            "forwarded into the quarantined subtree: exactly-once holds"
        );

        // Fully drained: the id is now recyclable.
        let id2 = rr.add_child(MockLeaf::ready_firing(3));
        assert_eq!(id2, id0);
    }

    #[test]
    fn idle_removal_detaches_and_recycles_the_id_immediately() {
        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let id0 = rr.add_child(MockLeaf::ready_firing(1));
        // Nothing in flight: the subtree is handed back for reuse and
        // the id frees at once, so unbounded add/remove churn cannot
        // exhaust the u32 tag space.
        assert!(matches!(
            rr.remove_child(id0),
            Some(RemovedChild::Detached(_))
        ));
        let id1 = rr.add_child(MockLeaf::ready_firing(2));
        assert_eq!(id1, id0);
    }

    #[test]
    fn completion_for_a_live_child_decrements_its_in_flight_count() {
        // Remove after the completion has already drained: the id must
        // free immediately (the slot's count went back to zero).
        let l0 = MockLeaf::ready_firing(1);
        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        let id0 = rr.add_child(l0);
        dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO)
            .expect("leaf fires");
        assert!(rr.remove_child(id0).is_some());
        let id1 = rr.add_child(MockLeaf::ready_firing(2));
        assert_eq!(id1, id0);
    }

    #[test]
    fn completion_routes_back_to_the_originating_child() {
        let l0 = MockLeaf::ready_idle(); // does not fire
        let l1 = MockLeaf::ready_firing(42); // fires
        let h0 = l0.handle();
        let h1 = l1.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        rr.add_child(l0);
        rr.add_child(l1);

        dispatch_and_complete(
            &mut rr,
            CompletionOutcome::Succeeded,
            Duration::from_millis(7),
        )
        .expect("l1 should fire");

        // The completion must have landed on l1, not l0.
        assert_eq!(h0.completion_count(), 0);
        assert_eq!(h1.completion_count(), 1);
        assert_eq!(
            h1.last_completion_outcome(),
            Some(CompletionOutcome::Succeeded)
        );
    }
}
