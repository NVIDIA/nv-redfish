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

//! Stochastic fair queue discipline.
//!
//! User metadata is classified into a flow key at enqueue. The key is
//! hashed into one of a fixed number of FIFO buckets; dequeue rotates
//! round-robin over non-empty buckets.

use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;

use super::bounded_queue::{QueueDiscipline, QueueEntryId};

/// Fixed-bucket stochastic fair queue.
pub struct StochasticFairQueue<C, F, H = RandomState> {
    classifier: C,
    hash_builder: H,
    buckets: Vec<VecDeque<QueueEntryId>>,
    active: VecDeque<usize>,
    membership: HashMap<QueueEntryId, usize>,
    _flow: PhantomData<fn() -> F>,
}

impl<C, F> StochasticFairQueue<C, F, RandomState> {
    /// Construct an SFQ with `bucket_count` and a metadata classifier.
    ///
    /// The classifier is called exactly once for each admitted item.
    /// Per-instance [`RandomState`] hashing protects against adversarial
    /// flow keys; use [`StochasticFairQueue::with_hash_builder`] when
    /// deterministic bucket assignment is required.
    #[must_use]
    pub fn new(bucket_count: NonZeroUsize, classifier: C) -> Self {
        Self::with_hash_builder(bucket_count, classifier, RandomState::new())
    }
}

impl<C, F, H> StochasticFairQueue<C, F, H> {
    /// Construct an SFQ with an explicit hash builder.
    ///
    /// This is useful for reproducible tests and deployments that require
    /// stable flow-to-bucket assignment.
    #[must_use]
    pub fn with_hash_builder(bucket_count: NonZeroUsize, classifier: C, hash_builder: H) -> Self {
        let buckets = (0..bucket_count.get()).map(|_| VecDeque::new()).collect();
        Self {
            classifier,
            hash_builder,
            buckets,
            active: VecDeque::new(),
            membership: HashMap::new(),
            _flow: PhantomData,
        }
    }

    fn bucket_for(&self, flow: &F) -> usize
    where
        F: Hash,
        H: BuildHasher,
    {
        let hash = self.hash_builder.hash_one(flow);
        #[allow(clippy::cast_possible_truncation)]
        let hash = hash as usize;
        hash % self.buckets.len()
    }
}

impl<M, C, F, H> QueueDiscipline<M> for StochasticFairQueue<C, F, H>
where
    C: FnMut(&M) -> F + Send + 'static,
    F: Hash + Send + 'static,
    H: BuildHasher + Send + 'static,
{
    fn push(&mut self, id: QueueEntryId, meta: &M) {
        let flow = (self.classifier)(meta);
        let bucket = self.bucket_for(&flow);
        let entries = self
            .buckets
            .get_mut(bucket)
            .expect("hashed SFQ bucket is in range");
        if entries.is_empty() {
            self.active.push_back(bucket);
        }
        entries.push_back(id);
        self.membership.insert(id, bucket);
    }

    fn take_next(&mut self) -> Option<QueueEntryId> {
        let bucket = self.active.pop_front()?;
        let entries = self.buckets.get_mut(bucket)?;
        let id = entries.pop_front()?;
        self.membership.remove(&id);
        if !entries.is_empty() {
            self.active.push_back(bucket);
        }
        Some(id)
    }

    fn remove(&mut self, id: QueueEntryId) -> bool {
        let Some(bucket) = self.membership.remove(&id) else {
            return false;
        };
        let entries = self
            .buckets
            .get_mut(bucket)
            .expect("indexed SFQ bucket is in range");
        let Some(position) = entries.iter().position(|&entry| entry == id) else {
            return false;
        };
        let _ = entries.remove(position);
        if entries.is_empty() {
            if let Some(active_position) = self.active.iter().position(|&active| active == bucket) {
                let _ = self.active.remove(active_position);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use core::hash::BuildHasherDefault;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::hash::DefaultHasher;
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    use super::StochasticFairQueue;
    use crate::scheduler::{ScheduledWork, Scheduler as _};
    use crate::schedulers::{BoundedQueueBuilder, QueueDiscipline as _, QueueEntryId};

    fn buckets(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("non-zero test bucket count")
    }

    #[test]
    fn alternates_non_empty_buckets_and_preserves_bucket_fifo() {
        let mut sfq = StochasticFairQueue::new(buckets(8), |flow: &u64| *flow);
        let first_flow = 0;
        let first_bucket = sfq.bucket_for(&first_flow);
        let second_flow = (1..10_000)
            .find(|flow| sfq.bucket_for(flow) != first_bucket)
            .expect("multiple buckets produce a distinct flow");

        sfq.push(QueueEntryId(1), &first_flow);
        sfq.push(QueueEntryId(2), &first_flow);
        sfq.push(QueueEntryId(3), &second_flow);
        sfq.push(QueueEntryId(4), &second_flow);

        assert_eq!(sfq.take_next(), Some(QueueEntryId(1)));
        assert_eq!(sfq.take_next(), Some(QueueEntryId(3)));
        assert_eq!(sfq.take_next(), Some(QueueEntryId(2)));
        assert_eq!(sfq.take_next(), Some(QueueEntryId(4)));
    }

    #[test]
    fn arbitrary_removal_keeps_rotation_consistent() {
        let mut sfq = StochasticFairQueue::new(buckets(1), |flow: &u8| *flow);
        sfq.push(QueueEntryId(1), &0);
        sfq.push(QueueEntryId(2), &0);
        sfq.push(QueueEntryId(3), &0);

        assert!(sfq.remove(QueueEntryId(2)));
        assert!(!sfq.remove(QueueEntryId(2)));
        assert_eq!(sfq.take_next(), Some(QueueEntryId(1)));
        assert_eq!(sfq.take_next(), Some(QueueEntryId(3)));
        assert_eq!(sfq.take_next(), None);
    }

    #[test]
    fn explicit_hash_builder_makes_bucket_assignment_reproducible() {
        type StableHasher = BuildHasherDefault<DefaultHasher>;

        let first = StochasticFairQueue::with_hash_builder(
            buckets(16),
            |flow: &u64| *flow,
            StableHasher::default(),
        );
        let second = StochasticFairQueue::with_hash_builder(
            buckets(16),
            |flow: &u64| *flow,
            StableHasher::default(),
        );
        for flow in 0..128 {
            assert_eq!(first.bucket_for(&flow), second.bucket_for(&flow));
        }
    }

    #[test]
    fn builder_classifies_each_admitted_item_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_classifier = calls.clone();
        let sfq = StochasticFairQueue::new(buckets(1), move |flow: &u8| {
            calls_for_classifier.fetch_add(1, Ordering::Relaxed);
            *flow
        });
        let (mut queue, producer) = BoundedQueueBuilder::new(buckets(4)).discipline(sfq).build();
        for flow in 0..3 {
            let _ = producer.try_push(ScheduledWork::new(flow, flow));
        }
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert_eq!(queue.take_next().map(|work| work.payload), Some(0));
        assert_eq!(queue.take_next().map(|work| work.payload), Some(1));
        assert_eq!(queue.take_next().map(|work| work.payload), Some(2));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }
}
