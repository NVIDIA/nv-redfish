// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;
use std::time::Duration;

use nv_redfish_core::ODataId;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::predicate::PredicateContext;
use crate::Error;
use crate::EventReceiver;
use crate::Predicate;
use crate::QueryError;
use crate::QueryId;
use crate::RelationEvent;
use crate::ResourceEvent;
use crate::ResourceSnapshot;
use crate::Scraper;
use crate::ScraperEvent;

/// Typed resource event emitted by a subscription.
#[derive(Clone, Debug)]
pub enum TypedResourceEvent<T> {
    /// A resource entered the subscription membership.
    Added(ResourceSnapshot<T>),
    /// A matching resource was updated.
    Updated {
        /// Previous matching snapshot, when tracked by this subscription.
        previous: Option<ResourceSnapshot<T>>,
        /// New matching snapshot.
        new: ResourceSnapshot<T>,
    },
    /// A resource left the subscription membership.
    Removed(ODataId),
    /// A member resource is older than requested freshness.
    FreshnessMissed {
        /// Resource `@odata.id`.
        id: ODataId,
        /// Current snapshot age.
        age: Duration,
        /// Desired freshness.
        desired: Duration,
    },
    /// A refresh for a member resource failed.
    Error {
        /// Resource `@odata.id`.
        id: ODataId,
        /// Refresh error.
        error: Arc<Error>,
    },
}

/// Typed subscription over matching resource changes.
pub struct TypedSubscription<B, T> {
    scraper: Scraper<B>,
    query_id: QueryId,
    predicates: Vec<Arc<dyn Predicate<T>>>,
    events: EventReceiver,
    pending: VecDeque<TypedResourceEvent<T>>,
    members: BTreeSet<ODataId>,
    task: Option<JoinHandle<()>>,
    active: bool,
}

impl<B, T> Debug for TypedSubscription<B, T>
where
    B: Debug,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("TypedSubscription")
            .field("scraper", &self.scraper)
            .field("predicates", &self.predicates.len())
            .field("pending", &self.pending.len())
            .field("members", &self.members)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl<B, T> TypedSubscription<B, T>
where
    B: Send + Sync,
    T: Send + Sync + 'static,
{
    pub(super) fn new(
        scraper: Scraper<B>,
        query_id: QueryId,
        predicates: Vec<Arc<dyn Predicate<T>>>,
        snapshots: Vec<ResourceSnapshot<T>>,
        task: Option<JoinHandle<()>>,
    ) -> Self {
        let events = scraper.subscribe_events();
        let mut pending = VecDeque::with_capacity(snapshots.len());
        let mut members = BTreeSet::new();
        for snapshot in snapshots {
            members.insert(snapshot.id.clone());
            pending.push_back(TypedResourceEvent::Added(snapshot));
        }
        Self {
            scraper,
            query_id,
            predicates,
            events,
            pending,
            members,
            task,
            active: true,
        }
    }

    /// Receives the next typed subscription event.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying global event stream is closed or
    /// this receiver lags behind the broadcast buffer.
    pub async fn recv(&mut self) -> Result<TypedResourceEvent<T>, Error> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(event);
        }
        loop {
            let envelope = self
                .events
                .recv()
                .await
                .map_err(|error| subscription_recv_error(&error))?;
            if let Some(event) = self.project_event(envelope.event) {
                return Ok(event);
            }
        }
    }

    fn project_event(&mut self, event: ScraperEvent) -> Option<TypedResourceEvent<T>> {
        match event {
            ScraperEvent::Resource(event) => self.project_resource_event(event),
            ScraperEvent::Relation(
                RelationEvent::Added { relation } | RelationEvent::Removed { relation },
            ) => self.project_mutation(relation.from.type_id, &relation.from.id),
            ScraperEvent::Discovery(_) | ScraperEvent::Scheduler(_) | ScraperEvent::Query(_) => {
                None
            }
        }
    }

    fn project_resource_event(&mut self, event: ResourceEvent) -> Option<TypedResourceEvent<T>> {
        match event {
            ResourceEvent::Added { type_id, id } | ResourceEvent::Updated { type_id, id } => {
                self.project_mutation(type_id, &id)
            }
            ResourceEvent::Error { type_id, id, error } => (type_id == TypeId::of::<T>()
                && self.members.contains(&id))
            .then_some(TypedResourceEvent::Error { id, error }),
            ResourceEvent::FreshnessMissed {
                type_id,
                id,
                age,
                desired,
            } => (type_id == TypeId::of::<T>() && self.members.contains(&id))
                .then_some(TypedResourceEvent::FreshnessMissed { id, age, desired }),
        }
    }

    fn project_mutation(&mut self, type_id: TypeId, id: &ODataId) -> Option<TypedResourceEvent<T>> {
        if type_id != TypeId::of::<T>() {
            return None;
        }
        let was_member = self.members.contains(id);
        let snapshot = self.scraper.resources::<T>().cached(id.clone())?;
        let matches = self.matches_candidate(id) && self.matches_snapshot(&snapshot);
        match (was_member, matches) {
            (false, true) => {
                self.members.insert(id.clone());
                Some(TypedResourceEvent::Added(snapshot))
            }
            (true, true) => Some(TypedResourceEvent::Updated {
                previous: None,
                new: snapshot,
            }),
            (true, false) => {
                self.members.remove(id);
                Some(TypedResourceEvent::Removed(id.clone()))
            }
            (false, false) => None,
        }
    }

    fn matches_candidate(&self, id: &ODataId) -> bool {
        self.predicates
            .iter()
            .all(|predicate| predicate.matches_candidate(id))
    }

    fn matches_snapshot(&self, snapshot: &ResourceSnapshot<T>) -> bool {
        let context = PredicateContext::new(&self.scraper.inner().store);
        self.predicates
            .iter()
            .all(|predicate| predicate.matches_snapshot(snapshot, &context))
    }
}

impl<B, T> Drop for TypedSubscription<B, T> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if self.active {
            self.scraper
                .inner()
                .queries
                .unregister_long_lived(self.query_id, &self.scraper.inner().events);
            let _ignored = self.scraper.inner().store.remove_query(self.query_id);
            self.active = false;
        }
    }
}

const fn subscription_recv_error(error: &broadcast::error::RecvError) -> Error {
    match error {
        broadcast::error::RecvError::Closed => Error::Query(QueryError::EventStreamClosed),
        broadcast::error::RecvError::Lagged(count) => {
            Error::Query(QueryError::EventStreamLagged(*count))
        }
    }
}
