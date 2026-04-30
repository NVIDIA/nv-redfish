// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeSet;
use std::time::Duration;

use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use tokio::task::JoinHandle;
use tokio::time::sleep_until;
use tokio::time::Instant;

use super::QueryBuilder;
use crate::Lane;
use crate::QueryId;
use crate::ResourceEvent;
use crate::ResourceSnapshot;
use crate::ScraperEvent;

pub(super) fn snapshot_ids<T>(snapshots: &[ResourceSnapshot<T>]) -> BTreeSet<ODataId> {
    snapshots
        .iter()
        .map(|snapshot| snapshot.id.clone())
        .collect()
}

pub(super) fn spawn_background_query<B, T>(
    query: &QueryBuilder<B, T>,
    query_id: QueryId,
    lane: Lane,
    members: BTreeSet<ODataId>,
) -> Option<JoinHandle<()>>
where
    B: Bmc + Send + Sync + 'static,
    B::Error: 'static,
    T: EntityTypeRef + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    if query.freshness.is_none() && query.discovery_freshness.is_none() {
        return None;
    }
    let task = BackgroundQuery {
        query: query.clone(),
        query_id,
        lane,
        next_resource_refresh: query.freshness.map(|freshness| Instant::now() + freshness),
        next_discovery_refresh: query
            .discovery_freshness
            .map(|freshness| Instant::now() + freshness),
        members,
    };
    Some(tokio::spawn(async move {
        task.run().await;
    }))
}

struct BackgroundQuery<B, T> {
    query: QueryBuilder<B, T>,
    query_id: QueryId,
    lane: Lane,
    next_resource_refresh: Option<Instant>,
    next_discovery_refresh: Option<Instant>,
    members: BTreeSet<ODataId>,
}

impl<B, T> BackgroundQuery<B, T>
where
    B: Bmc + Send + Sync + 'static,
    B::Error: 'static,
    T: EntityTypeRef + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn run(mut self) {
        loop {
            let Some(deadline) = self.next_deadline() else {
                return;
            };
            sleep_until(deadline).await;
            self.reconcile().await;
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        match (self.next_resource_refresh, self.next_discovery_refresh) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
            (None, None) => None,
        }
    }

    async fn reconcile(&mut self) {
        let now = Instant::now();
        if self
            .next_discovery_refresh
            .is_some_and(|deadline| now >= deadline)
        {
            self.refresh_discovery().await;
            self.next_discovery_refresh = self
                .query
                .discovery_freshness
                .map(|freshness| Instant::now() + freshness);
        }
        if self
            .next_resource_refresh
            .is_some_and(|deadline| now >= deadline)
        {
            self.refresh_stale_members().await;
            self.next_resource_refresh = self
                .query
                .freshness
                .map(|freshness| Instant::now() + freshness);
        }
    }

    async fn refresh_discovery(&mut self) {
        let Ok(candidates) = self.query.discover_candidate_ids().await else {
            return;
        };
        let mut members = BTreeSet::new();
        for id in candidates
            .into_iter()
            .filter(|id| self.query.matches_candidate(id))
        {
            let refreshed = if self.should_refresh_candidate(&id) {
                self.query
                    .scraper
                    .resources::<T>()
                    .refresh_with_lane(id.clone(), self.lane)
                    .await
                    .ok()
            } else {
                None
            };
            let snapshot = refreshed.or_else(|| self.query.scraper.resources::<T>().cached(id));
            if let Some(snapshot) = snapshot {
                if self.query.matches_snapshot(&snapshot) {
                    members.insert(snapshot.id);
                }
            }
        }
        self.members = members;
        let snapshots = self
            .members
            .iter()
            .filter_map(|id| self.query.scraper.resources::<T>().cached(id.clone()))
            .collect::<Vec<_>>();
        let _ignored = self.query.update_query_members(self.query_id, &snapshots);
    }

    async fn refresh_stale_members(&mut self) {
        let Some(freshness) = self.query.freshness else {
            return;
        };
        for id in self.members.clone() {
            let Some(snapshot) = self.query.scraper.resources::<T>().cached(id.clone()) else {
                self.members.remove(&id);
                continue;
            };
            if !self.query.matches_snapshot(&snapshot) {
                self.members.remove(&id);
                continue;
            }
            if !snapshot.is_stale_for(freshness) {
                continue;
            }
            self.publish_freshness_missed(&snapshot, freshness);
            match self
                .query
                .scraper
                .resources::<T>()
                .refresh_with_lane(id.clone(), self.lane)
                .await
            {
                Ok(snapshot) if self.query.matches_snapshot(&snapshot) => {
                    self.members.insert(snapshot.id);
                }
                Ok(_) => {
                    self.members.remove(&id);
                }
                Err(_) => {}
            }
        }
        let snapshots = self
            .members
            .iter()
            .filter_map(|id| self.query.scraper.resources::<T>().cached(id.clone()))
            .collect::<Vec<_>>();
        let _ignored = self.query.update_query_members(self.query_id, &snapshots);
    }

    fn should_refresh_candidate(&self, id: &ODataId) -> bool {
        self.query
            .scraper
            .resources::<T>()
            .cached(id.clone())
            .is_none_or(|snapshot| {
                self.query
                    .freshness
                    .is_some_and(|freshness| snapshot.is_stale_for(freshness))
            })
    }

    fn publish_freshness_missed(&self, snapshot: &ResourceSnapshot<T>, desired: Duration) {
        self.query
            .scraper
            .inner()
            .events
            .publish(ScraperEvent::Resource(ResourceEvent::FreshnessMissed {
                type_id: TypeId::of::<T>(),
                id: snapshot.id.clone(),
                age: snapshot.age(),
                desired,
            }));
    }
}
