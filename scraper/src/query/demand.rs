// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use crate::DiscoveryHint;
use crate::Error;
use crate::EventBus;
use crate::Lane;
use crate::QueryError;
use crate::QueryEvent;
use crate::ResourceRef;
use crate::ScraperEvent;

/// Stable identifier for an active query plan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QueryId(u64);

impl QueryId {
    const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric identifier.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Query demand lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryKind {
    /// One-shot query demand.
    Temporary,
    /// Subscription or watch demand.
    LongLived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Priority {
    Interactive,
    Background,
}

/// Active query plan tracked by the scraper.
#[derive(Clone, Debug)]
pub struct QueryPlan {
    /// Stable query identifier.
    pub id: QueryId,
    /// Query demand lifetime.
    pub kind: QueryKind,
    /// Rust type id for the queried resource type.
    pub type_id: TypeId,
    /// Discovery hint derived from query predicates.
    pub discovery_hint: DiscoveryHint,
    /// Desired resource freshness.
    pub freshness: Option<Duration>,
    /// Desired discovery freshness.
    pub discovery_freshness: Option<Duration>,
    /// Scheduler lane used by ongoing work for this plan.
    pub lane: Lane,
    /// Currently matching resource references.
    pub members: BTreeSet<ResourceRef>,
    priority: Priority,
}

impl QueryPlan {
    pub(super) const fn new(
        kind: QueryKind,
        type_id: TypeId,
        discovery_hint: DiscoveryHint,
        freshness: Option<Duration>,
        discovery_freshness: Option<Duration>,
        lane: Lane,
        priority: Priority,
    ) -> Self {
        Self {
            id: QueryId::new(0),
            kind,
            type_id,
            discovery_hint,
            freshness,
            discovery_freshness,
            lane,
            members: BTreeSet::new(),
            priority,
        }
    }

    const fn priority(&self) -> Priority {
        self.priority
    }
}

/// Tracks active query demand.
#[derive(Debug)]
pub struct QueryManager {
    state: Mutex<QueryState>,
}

impl Default for QueryManager {
    fn default() -> Self {
        Self {
            state: Mutex::new(QueryState {
                next_id: 1,
                plans: BTreeMap::new(),
            }),
        }
    }
}

impl QueryManager {
    pub(super) fn register_temporary<'a>(
        &'a self,
        plan: QueryPlan,
        events: &'a EventBus,
    ) -> Result<TemporaryDemand<'a>, Error> {
        let id = self.register(plan, events)?;
        Ok(TemporaryDemand {
            manager: self,
            events,
            id,
            active: true,
        })
    }

    pub(super) fn register_long_lived(
        &self,
        plan: QueryPlan,
        events: &EventBus,
    ) -> Result<QueryId, Error> {
        self.register(plan, events)
    }

    fn register(&self, mut plan: QueryPlan, events: &EventBus) -> Result<QueryId, Error> {
        let (id, kind, type_id) = {
            let mut state = self.state.lock().map_err(Error::query_lock)?;
            let id = QueryId::new(state.next_id);
            state.next_id += 1;
            plan.id = id;
            let kind = plan.kind;
            let type_id = plan.type_id;
            let priority = plan.priority();
            debug_assert!(matches!(
                priority,
                Priority::Interactive | Priority::Background
            ));
            state.plans.insert(id, plan);
            drop(state);
            (id, kind, type_id)
        };
        events.publish(ScraperEvent::Query(QueryEvent::Registered {
            id,
            kind,
            type_id,
        }));
        Ok(id)
    }

    pub(super) fn unregister_long_lived(&self, id: QueryId, events: &EventBus) {
        self.remove(id, events);
    }

    pub(super) fn update_members(
        &self,
        id: QueryId,
        members: BTreeSet<ResourceRef>,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(Error::query_lock)?;
        let plan = state
            .plans
            .get_mut(&id)
            .ok_or(Error::Query(QueryError::UnknownQuery(id)))?;
        plan.members = members;
        drop(state);
        Ok(())
    }

    fn remove(&self, id: QueryId, events: &EventBus) {
        let removed = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.plans.remove(&id));
        if let Some(plan) = removed {
            events.publish(ScraperEvent::Query(QueryEvent::Removed {
                id,
                kind: plan.kind,
                type_id: plan.type_id,
            }));
        }
    }

    #[cfg(test)]
    pub(crate) fn active_temporary(&self) -> Result<usize, Error> {
        self.active_count(QueryKind::Temporary)
    }

    #[cfg(test)]
    pub(crate) fn active_long_lived(&self) -> Result<usize, Error> {
        self.active_count(QueryKind::LongLived)
    }

    #[cfg(test)]
    pub(crate) fn plan(&self, id: QueryId) -> Result<Option<QueryPlan>, Error> {
        self.state
            .lock()
            .map(|state| state.plans.get(&id).cloned())
            .map_err(Error::query_lock)
    }

    #[cfg(test)]
    fn active_count(&self, kind: QueryKind) -> Result<usize, Error> {
        self.state
            .lock()
            .map(|state| {
                state
                    .plans
                    .values()
                    .filter(|plan| plan.kind == kind)
                    .count()
            })
            .map_err(Error::query_lock)
    }
}

#[derive(Debug)]
struct QueryState {
    next_id: u64,
    plans: BTreeMap<QueryId, QueryPlan>,
}

pub(super) struct TemporaryDemand<'a> {
    manager: &'a QueryManager,
    events: &'a EventBus,
    id: QueryId,
    active: bool,
}

impl TemporaryDemand<'_> {
    pub(super) const fn id(&self) -> QueryId {
        self.id
    }
}

impl Drop for TemporaryDemand<'_> {
    fn drop(&mut self) {
        if self.active {
            self.manager.remove(self.id, self.events);
            self.active = false;
        }
    }
}
