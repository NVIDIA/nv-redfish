// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Demand-driven Redfish scraper.
//!
//! This crate maintains a typed, freshness-aware view of Redfish resources above
//! [`nv_redfish_core::Bmc`]. The current implementation supports direct typed
//! refresh for caller-provided resource URIs and one-shot queries backed by
//! manually registered discoverers. Subscriptions, polling, and adaptive
//! scheduling are added in later phases.

#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::unimplemented,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![deny(missing_docs)]
#![allow(clippy::doc_markdown)]

mod builder;
mod capacity;
mod discovery;
mod error;
mod event;
/// Typed query predicates.
pub mod predicate;
mod query;
mod raw;
mod relation;
mod resources;
mod scheduler;
mod snapshot;
mod store;

use std::sync::Arc;

pub use builder::ScraperBuilder;
pub use capacity::BmcCapacity;
pub use discovery::Discoverer;
pub use discovery::Discovery;
pub use discovery::DiscoveryBatch;
pub use discovery::DiscoveryContext;
pub use discovery::DiscoveryEvent;
pub use discovery::DiscoveryHint;
pub use discovery::DiscoverySourceId;
pub use error::DiscoveryError;
pub use error::Error;
pub use error::QueryError;
pub use error::SchedulerError;
pub use error::StoreError;
pub use event::EventEnvelope;
pub use event::EventReceiver;
pub use event::EventSeq;
pub use event::QueryEvent;
pub use event::RelationEvent;
pub use event::ResourceEvent;
pub use event::ScraperEvent;
pub use predicate::Predicate;
pub use predicate::PredicateContext;
pub use query::QueryBuilder;
pub use query::QueryId;
pub use query::QueryKind;
pub use query::QueryPlan;
pub use query::QueryWatch;
pub use query::TypedResourceEvent;
pub use query::TypedSubscription;
pub use raw::RawResource;
pub use raw::RawSnapshot;
pub use relation::Relation;
pub use relation::RelationKind;
pub use relation::ResourceRef;
pub use resources::ResourceClient;
pub use scheduler::Lane;
pub use scheduler::LoadState;
pub use scheduler::SchedulerEvent;
pub use scheduler::SchedulerStats;
pub use snapshot::ResourceSnapshot;
pub use snapshot::Staleness;

use discovery::DiscoveryRegistry;
use event::EventBus;
use query::QueryManager;
use resources::RefreshCoalescer;
use scheduler::Scheduler;
use store::ResourceStore;

/// Shared Redfish scraper handle.
///
/// The handle is cheap to clone. Construction stores the supplied BMC client
/// and shared scraper state without issuing BMC requests.
#[derive(Debug)]
pub struct Scraper<B> {
    inner: Arc<Inner<B>>,
}

impl<B> Clone for Scraper<B> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<B> Scraper<B> {
    /// Starts a scraper builder around `bmc`.
    ///
    /// This does not perform BMC I/O.
    #[must_use]
    pub fn builder(bmc: B) -> ScraperBuilder<B> {
        ScraperBuilder::new(bmc)
    }

    /// Returns a direct resource client for type `T`.
    ///
    /// Refresh operations issued through this handle may perform BMC I/O through
    /// the scheduler. Cached reads inspect the local store only.
    #[must_use]
    pub fn resources<T>(&self) -> ResourceClient<B, T> {
        ResourceClient::new(self.clone())
    }

    /// Returns a direct raw resource client for unknown or OEM resources.
    ///
    /// Refresh operations still perform BMC I/O through the scheduler. Cached
    /// reads inspect the local store only.
    #[must_use]
    pub fn raw_resources(&self) -> ResourceClient<B, RawResource> {
        self.resources::<RawResource>()
    }

    /// Returns a typed query builder for type `T`.
    ///
    /// One-shot listing may perform discovery and BMC I/O through registered
    /// discoverers and the scheduler.
    #[must_use]
    pub fn query<T>(&self) -> QueryBuilder<B, T>
    where
        T: 'static,
    {
        QueryBuilder::new(self.clone())
    }

    /// Subscribes to future scraper events.
    ///
    /// Receivers observe future resource and scheduler events from this scraper
    /// instance.
    #[must_use]
    pub fn subscribe_events(&self) -> EventReceiver {
        self.inner.events.subscribe()
    }

    pub(crate) fn from_parts(bmc: B, capacity: BmcCapacity, discovery: DiscoveryRegistry) -> Self {
        Self {
            inner: Arc::new(Inner {
                bmc,
                _capacity: capacity,
                discovery,
                events: EventBus::default(),
                queries: QueryManager::default(),
                refreshes: RefreshCoalescer::default(),
                scheduler: Scheduler::new(capacity),
                store: ResourceStore::default(),
            }),
        }
    }

    pub(crate) fn inner(&self) -> &Inner<B> {
        &self.inner
    }

    /// Records a direct relation and emits a relation event when it is new.
    ///
    /// Discoverers normally provide relation metadata through
    /// [`DiscoveryBatch`]. This method is available for callers that already
    /// know relation metadata out of band.
    ///
    /// # Errors
    ///
    /// Returns an error when the relation index cannot be updated.
    pub fn record_relation(&self, relation: Relation) -> Result<(), Error> {
        if self.inner.store.insert_relation(relation.clone())? {
            self.inner
                .events
                .publish(ScraperEvent::Relation(RelationEvent::Added { relation }));
        }
        Ok(())
    }

    /// Removes a direct relation and emits a relation event when it existed.
    ///
    /// # Errors
    ///
    /// Returns an error when the relation index cannot be updated.
    pub fn remove_relation(&self, relation: &Relation) -> Result<(), Error> {
        if self.inner.store.remove_relation(relation)? {
            self.inner
                .events
                .publish(ScraperEvent::Relation(RelationEvent::Removed {
                    relation: relation.clone(),
                }));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Inner<B> {
    bmc: B,
    _capacity: BmcCapacity,
    discovery: DiscoveryRegistry,
    events: EventBus,
    queries: QueryManager,
    refreshes: RefreshCoalescer,
    scheduler: Scheduler,
    store: ResourceStore,
}

#[cfg(test)]
mod tests;
