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

//! Plain round-robin branch.
//!
//! Children are stored in a `Vec` (stable indices used as the routing
//! tag). Iteration order is a `VecDeque<u32>` of indices: `take_next`
//! pops the front, asks that child, pushes the index back regardless of
//! result, and stops on the first `Some`. New children appended mid-scan
//! land at the back of the queue and are visited within one cycle.

use core::convert::TryFrom as _;
use core::marker::PhantomData;
use std::collections::VecDeque;
use std::time::Instant;

use crate::scheduler::{ScheduledWork, Scheduler};
use crate::work::{Completion, Readiness, WorkMeta};

/// Round robing over boxed children
pub struct RoundRobin<T, M: WorkMeta> {
    children: Vec<Box<dyn Scheduler<T, Meta = M>>>,
    queue: VecDeque<u32>,
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
    pub const fn new() -> Self {
        Self {
            children: Vec::new(),
            queue: VecDeque::new(),
            _t: PhantomData,
        }
    }

    /// Append `child` and return its stable index (the routing tag).
    ///
    /// # Panics
    ///
    /// Panics if more than `u32::MAX` children are added (which the
    /// [`RoutingPath`](crate::RoutingPath) tag width does not support).
    pub fn add_child<S>(&mut self, child: S) -> u32
    where
        S: Scheduler<T, Meta = M>,
    {
        let id = u32::try_from(self.children.len())
            .expect("RoundRobin supports up to u32::MAX children");
        self.children.push(Box::new(child));
        self.queue.push_back(id);
        id
    }

    /// Number of children currently held by this branch.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.children.len()
    }

    /// `true` when no children have been added yet.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
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
        for child in &mut self.children {
            let r = child.update_ready(now);
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
            self.queue.push_back(id);
            let idx = usize::try_from(id).ok()?;
            if let Some(mut work) = self.children[idx].take_next() {
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
        let idx = usize::try_from(id).expect("u32 stable index fits in usize");
        if let Some(child) = self.children.get_mut(idx) {
            child.on_complete(completion);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use core::time::Duration;
    use std::time::Instant;

    use super::RoundRobin;
    use crate::scheduler::Scheduler as _;
    use crate::schedulers::tests::{dispatch_and_complete, MockLeaf};
    use crate::work::CompletionOutcome;

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
        let leaf = MockLeaf::ready_firing(0, 11);
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
        let l0 = MockLeaf::ready_firing(0, 100);
        let l1 = MockLeaf::ready_firing(1, 200);
        let l2 = MockLeaf::ready_firing(2, 300);
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
        let l0 = MockLeaf::ready_firing(0, 1);
        let l1 = MockLeaf::ready_idle(1); // ready=true but no payload
        let l2 = MockLeaf::ready_firing(2, 3);
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
        let l0 = MockLeaf::ready_firing(0, 1);
        let l1 = MockLeaf::ready_firing(1, 2);
        let h0 = l0.handle();
        let h1 = l1.handle();

        let mut rr: RoundRobin<u64, ()> = RoundRobin::new();
        rr.add_child(l0);
        rr.add_child(l1);

        // Pull one item to advance the cursor.
        dispatch_and_complete(&mut rr, CompletionOutcome::Succeeded, Duration::ZERO).expect("ok");

        // Add a third child mid-stream.
        let l2 = MockLeaf::ready_firing(2, 3);
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
    fn completion_routes_back_to_the_originating_child() {
        let l0 = MockLeaf::ready_idle(0); // does not fire
        let l1 = MockLeaf::ready_firing(1, 42); // fires
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
