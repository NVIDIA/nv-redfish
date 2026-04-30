// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use nv_redfish_core::ODataId;
use tokio::sync::broadcast;

use crate::DiscoveryEvent;
use crate::Error;
use crate::QueryId;
use crate::QueryKind;
use crate::Relation;
use crate::SchedulerEvent;

/// Stream-like receiver for scraper events.
///
/// Receivers observe events emitted after subscription.
pub type EventReceiver = broadcast::Receiver<EventEnvelope>;

/// Sequenced scraper event.
#[derive(Clone, Debug)]
pub struct EventEnvelope {
    /// Monotonic sequence number within one scraper instance.
    pub seq: EventSeq,
    /// Time when the event was published.
    pub timestamp: SystemTime,
    /// Event payload.
    pub event: ScraperEvent,
}

/// Monotonic event sequence number within one scraper instance.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EventSeq(u64);

impl EventSeq {
    const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric sequence value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Top-level scraper event.
#[derive(Clone, Debug)]
pub enum ScraperEvent {
    /// Discovery event.
    Discovery(DiscoveryEvent),
    /// Relation index event.
    Relation(RelationEvent),
    /// Resource store or refresh event.
    Resource(ResourceEvent),
    /// Scheduler state event.
    Scheduler(SchedulerEvent),
    /// Query lifecycle event.
    Query(QueryEvent),
}

/// Query lifecycle event emitted by the scraper.
#[derive(Clone, Debug)]
pub enum QueryEvent {
    /// A query plan was registered.
    Registered {
        /// Stable query identifier.
        id: QueryId,
        /// Query demand lifetime.
        kind: QueryKind,
        /// Rust type id for the queried resource type.
        type_id: TypeId,
    },
    /// A query plan was removed.
    Removed {
        /// Stable query identifier.
        id: QueryId,
        /// Query demand lifetime.
        kind: QueryKind,
        /// Rust type id for the queried resource type.
        type_id: TypeId,
    },
}

/// Relation event emitted by the scraper.
#[derive(Clone, Debug)]
pub enum RelationEvent {
    /// A relation was inserted into the relation index.
    Added {
        /// Inserted relation.
        relation: Relation,
    },
    /// A relation was removed from the relation index.
    Removed {
        /// Removed relation.
        relation: Relation,
    },
}

/// Resource event emitted by the scraper.
#[derive(Clone, Debug)]
pub enum ResourceEvent {
    /// A resource was inserted into the store.
    Added {
        /// Rust type id for the resource type.
        type_id: TypeId,
        /// Resource `@odata.id`.
        id: ODataId,
    },
    /// A resource already present in the store was refreshed.
    Updated {
        /// Rust type id for the resource type.
        type_id: TypeId,
        /// Resource `@odata.id`.
        id: ODataId,
    },
    /// A resource refresh failed.
    Error {
        /// Rust type id for the resource type.
        type_id: TypeId,
        /// Resource `@odata.id`.
        id: ODataId,
        /// Refresh error.
        error: Arc<Error>,
    },
    /// A resource is older than requested freshness.
    FreshnessMissed {
        /// Rust type id for the resource type.
        type_id: TypeId,
        /// Resource `@odata.id`.
        id: ODataId,
        /// Current snapshot age.
        age: Duration,
        /// Desired freshness.
        desired: Duration,
    },
}

#[derive(Debug)]
pub struct EventBus {
    sender: broadcast::Sender<EventEnvelope>,
    next_seq: AtomicU64,
}

impl Default for EventBus {
    fn default() -> Self {
        let (sender, _receiver) = broadcast::channel(128);
        Self {
            sender,
            next_seq: AtomicU64::new(1),
        }
    }
}

impl EventBus {
    pub fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }

    pub(crate) fn publish(&self, event: ScraperEvent) {
        let seq = EventSeq::new(self.next_seq.fetch_add(1, Ordering::Relaxed));
        let envelope = EventEnvelope {
            seq,
            timestamp: SystemTime::now(),
            event,
        };
        let _ignored_receiver_count = self.sender.send(envelope);
    }
}
