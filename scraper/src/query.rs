// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use serde::Deserialize;

use crate::predicate::PredicateContext;
use crate::DiscoveryHint;
use crate::Error;
use crate::Lane;
use crate::Predicate;
use crate::ResourceRef;
use crate::ResourceSnapshot;
use crate::Scraper;

mod demand;
mod reconcile;
mod subscription;
mod watch;

use demand::Priority;
pub use demand::QueryId;
pub use demand::QueryKind;
pub use demand::QueryManager;
pub use demand::QueryPlan;
use reconcile::snapshot_ids;
use reconcile::spawn_background_query;
pub use subscription::TypedResourceEvent;
pub use subscription::TypedSubscription;
pub use watch::QueryWatch;

/// Typed query builder.
///
/// One-shot queries discover candidate ids, refresh them through the scheduler,
/// and return typed snapshots. Predicates, watches, and subscriptions are added
/// in later phases.
pub struct QueryBuilder<B, T> {
    scraper: Scraper<B>,
    predicates: Vec<Arc<dyn Predicate<T>>>,
    freshness: Option<Duration>,
    discovery_freshness: Option<Duration>,
    resource_type: PhantomData<fn() -> T>,
}

impl<B, T> Debug for QueryBuilder<B, T>
where
    B: Debug,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("QueryBuilder")
            .field("scraper", &self.scraper)
            .field("predicates", &self.predicates.len())
            .field("freshness", &self.freshness)
            .field("discovery_freshness", &self.discovery_freshness)
            .finish()
    }
}

impl<B, T> Clone for QueryBuilder<B, T> {
    fn clone(&self) -> Self {
        Self {
            scraper: self.scraper.clone(),
            predicates: self.predicates.clone(),
            freshness: self.freshness,
            discovery_freshness: self.discovery_freshness,
            resource_type: PhantomData,
        }
    }
}

impl<B, T> QueryBuilder<B, T>
where
    T: 'static,
{
    pub(crate) fn new(scraper: Scraper<B>) -> Self {
        Self {
            scraper,
            predicates: Vec::new(),
            freshness: None,
            discovery_freshness: None,
            resource_type: PhantomData,
        }
    }

    /// Adds a typed predicate to this query.
    #[must_use]
    pub fn where_<P>(mut self, predicate: P) -> Self
    where
        P: Predicate<T>,
    {
        self.predicates.push(Arc::new(predicate));
        self
    }

    /// Sets desired resource freshness for subscriptions and watches.
    #[must_use]
    pub const fn freshness(mut self, freshness: Duration) -> Self {
        self.freshness = Some(freshness);
        self
    }

    /// Sets desired discovery freshness for subscriptions and watches.
    #[must_use]
    pub const fn discovery_freshness(mut self, freshness: Duration) -> Self {
        self.discovery_freshness = Some(freshness);
        self
    }

    /// Discovers and refreshes a typed resource set once.
    ///
    /// Returns an empty vector when no discoverer is registered for `T`.
    ///
    /// # Errors
    ///
    /// Returns an error when discovery fails or when refreshing a discovered
    /// candidate fails.
    pub async fn list(&self) -> Result<Vec<ResourceSnapshot<T>>, Error>
    where
        B: Bmc + Send + Sync + 'static,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let plan = self.plan(QueryKind::Temporary, Lane::Interactive);
        let demand = self
            .scraper
            .inner()
            .queries
            .register_temporary(plan, &self.scraper.inner().events)?;
        self.list_for_plan(demand.id(), Lane::Interactive).await
    }

    /// Subscribes to matching typed resource changes.
    ///
    /// The subscription first runs a one-shot list and yields those snapshots
    /// as initial `Added` events. It then observes the global scraper event
    /// stream and projects matching resource changes into typed events.
    ///
    /// # Errors
    ///
    /// Returns an error when registering demand fails or when the initial list
    /// fails.
    pub async fn subscribe(&self) -> Result<TypedSubscription<B, T>, Error>
    where
        B: Bmc + Send + Sync + 'static,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        let query_id = self.scraper.inner().queries.register_long_lived(
            self.plan(QueryKind::LongLived, Lane::Subscription),
            &self.scraper.inner().events,
        )?;
        let snapshots = match self.list_for_plan(query_id, Lane::Interactive).await {
            Ok(snapshots) => snapshots,
            Err(error) => {
                self.scraper
                    .inner()
                    .queries
                    .unregister_long_lived(query_id, &self.scraper.inner().events);
                let _ignored = self.scraper.inner().store.remove_query(query_id);
                return Err(error);
            }
        };
        let members = snapshot_ids(&snapshots);
        Ok(TypedSubscription::new(
            self.scraper.clone(),
            query_id,
            self.predicates.clone(),
            snapshots,
            spawn_background_query(self, query_id, Lane::Subscription, members),
        ))
    }

    /// Starts a background watch for matching resources.
    ///
    /// The watch performs an initial list to establish membership and then keeps
    /// matching resources warm according to the configured freshness settings.
    ///
    /// # Errors
    ///
    /// Returns an error when registering demand fails or the initial list fails.
    pub async fn watch(&self) -> Result<QueryWatch<B, T>, Error>
    where
        B: Bmc + Send + Sync + 'static,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        let query_id = self.scraper.inner().queries.register_long_lived(
            self.plan(QueryKind::LongLived, Lane::Maintenance),
            &self.scraper.inner().events,
        )?;
        let snapshots = match self.list_for_plan(query_id, Lane::Interactive).await {
            Ok(snapshots) => snapshots,
            Err(error) => {
                self.scraper
                    .inner()
                    .queries
                    .unregister_long_lived(query_id, &self.scraper.inner().events);
                let _ignored = self.scraper.inner().store.remove_query(query_id);
                return Err(error);
            }
        };
        let members = snapshot_ids(&snapshots);
        Ok(QueryWatch::<B, T> {
            scraper: self.scraper.clone(),
            query_id,
            task: spawn_background_query(self, query_id, Lane::Maintenance, members),
            active: true,
            resource_type: PhantomData,
        })
    }

    async fn list_for_plan(
        &self,
        query_id: QueryId,
        lane: Lane,
    ) -> Result<Vec<ResourceSnapshot<T>>, Error>
    where
        B: Bmc + Send + Sync + 'static,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let candidates = self.discover_candidate_ids().await?;
        let snapshots = self
            .refresh_candidates(candidates, lane, self.freshness)
            .await?;
        self.update_query_members(query_id, &snapshots)?;
        Ok(snapshots)
    }

    async fn discover_candidate_ids(&self) -> Result<Vec<ODataId>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: Send + Sync + 'static,
    {
        let hint = self.discovery_hint();
        let report = self
            .scraper
            .inner()
            .discovery
            .discover::<B, T>(
                &self.scraper.inner().bmc,
                &self.scraper.inner().scheduler,
                &self.scraper.inner().events,
                hint,
            )
            .await?;
        for source in &report.sources {
            self.scraper.inner().store.record_discovery_candidates(
                source.source_id,
                TypeId::of::<T>(),
                source.ids.clone(),
            )?;
        }
        for relation in report.batch.relations {
            self.scraper.record_relation(relation)?;
        }
        Ok(report.batch.candidates)
    }

    async fn refresh_candidates(
        &self,
        candidates: Vec<ODataId>,
        lane: Lane,
        desired: Option<Duration>,
    ) -> Result<Vec<ResourceSnapshot<T>>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let mut snapshots = Vec::with_capacity(candidates.len());
        for id in candidates
            .into_iter()
            .filter(|id| self.matches_candidate(id))
        {
            let snapshot = self
                .scraper
                .resources::<T>()
                .refresh_with_lane(id, lane)
                .await?
                .with_desired_freshness(desired);
            if self.matches_snapshot(&snapshot) {
                snapshots.push(snapshot);
            }
        }
        Ok(snapshots)
    }

    fn discovery_hint(&self) -> DiscoveryHint {
        let mut hint = DiscoveryHint::default();
        for predicate in &self.predicates {
            if let Some(next) = predicate.candidate_hint() {
                hint.merge(next);
            }
        }
        hint
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

    pub(super) fn update_query_members(
        &self,
        query_id: QueryId,
        snapshots: &[ResourceSnapshot<T>],
    ) -> Result<(), Error>
    where
        T: 'static,
    {
        let members = snapshots
            .iter()
            .map(|snapshot| ResourceRef::of::<T>(snapshot.id.clone()))
            .collect::<BTreeSet<_>>();
        self.scraper
            .inner()
            .queries
            .update_members(query_id, members.clone())?;
        self.scraper
            .inner()
            .store
            .set_query_members(query_id, members)?;
        Ok(())
    }

    fn plan(&self, kind: QueryKind, lane: Lane) -> QueryPlan {
        let priority = match lane {
            Lane::Interactive => Priority::Interactive,
            Lane::Subscription | Lane::Discovery | Lane::Maintenance => Priority::Background,
        };
        QueryPlan::new(
            kind,
            TypeId::of::<T>(),
            self.discovery_hint(),
            self.freshness,
            self.discovery_freshness,
            lane,
            priority,
        )
    }
}
