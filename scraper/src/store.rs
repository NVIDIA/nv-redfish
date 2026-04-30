// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::any::TypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use tokio::time::Instant;

use crate::DiscoverySourceId;
use crate::Error;
use crate::QueryId;
use crate::Relation;
use crate::ResourceRef;
use crate::ResourceSnapshot;
use crate::Staleness;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ResourceKey {
    type_id: TypeId,
    id: ODataId,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct RelationTypeKey {
    from_type: TypeId,
    to_type: TypeId,
}

#[derive(Clone)]
struct ErasedSnapshot {
    id: ODataId,
    etag: Option<ODataETag>,
    fetched_at: SystemTime,
    observed_at: Instant,
    staleness: Staleness,
    value: Arc<dyn Any + Send + Sync>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertStatus {
    Added,
    Updated,
}

/// Type-indexed in-memory resource store.
#[derive(Default)]
pub struct ResourceStore {
    inner: Mutex<ResourceStoreInner>,
}

impl Debug for ResourceStore {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("ResourceStore")
    }
}

impl ResourceStore {
    pub(crate) fn insert<T>(&self, snapshot: ResourceSnapshot<T>) -> Result<InsertStatus, Error>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = snapshot.id.clone();
        let key = ResourceKey {
            type_id,
            id: id.clone(),
        };
        let erased = ErasedSnapshot {
            id: snapshot.id,
            etag: snapshot.etag,
            fetched_at: snapshot.fetched_at,
            observed_at: snapshot.observed_at,
            staleness: snapshot.staleness,
            value: snapshot.value,
        };
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        let status = if inner.resources.insert(key, erased).is_some() {
            InsertStatus::Updated
        } else {
            InsertStatus::Added
        };
        inner.by_type.entry(type_id).or_default().insert(id);
        drop(inner);
        Ok(status)
    }

    pub(crate) fn get<T>(&self, id: &ODataId) -> Option<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        let key = ResourceKey {
            type_id: TypeId::of::<T>(),
            id: id.clone(),
        };
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.resources.get(&key).and_then(Self::typed_snapshot))
    }

    pub(crate) fn list<T>(&self) -> Vec<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let Ok(inner) = self.inner.lock() else {
            return Vec::new();
        };
        inner
            .by_type
            .get(&type_id)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| {
                let key = ResourceKey {
                    type_id,
                    id: id.clone(),
                };
                inner.resources.get(&key).and_then(Self::typed_snapshot)
            })
            .collect()
    }

    pub(crate) fn insert_relation(&self, relation: Relation) -> Result<bool, Error> {
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        if !inner.relations.insert(relation.clone()) {
            return Ok(false);
        }
        inner
            .relations_by_from
            .entry(relation.from.clone())
            .or_default()
            .insert(relation.clone());
        inner
            .relations_by_type
            .entry(RelationTypeKey {
                from_type: relation.from.type_id,
                to_type: relation.to.type_id,
            })
            .or_default()
            .insert(relation);
        drop(inner);
        Ok(true)
    }

    pub(crate) fn set_query_members(
        &self,
        query_id: QueryId,
        members: BTreeSet<ResourceRef>,
    ) -> Result<(), Error> {
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        inner.by_query.insert(query_id, members);
        drop(inner);
        Ok(())
    }

    pub(crate) fn remove_query(&self, query_id: QueryId) -> Result<(), Error> {
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        inner.by_query.remove(&query_id);
        drop(inner);
        Ok(())
    }

    pub(crate) fn record_discovery_candidates(
        &self,
        source_id: DiscoverySourceId,
        type_id: TypeId,
        ids: impl IntoIterator<Item = ODataId>,
    ) -> Result<(), Error> {
        let refs = ids
            .into_iter()
            .map(|id| ResourceRef::new(type_id, id))
            .collect::<BTreeSet<_>>();
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        inner.by_discovery_source.insert(source_id, refs);
        drop(inner);
        Ok(())
    }

    pub(crate) fn remove_relation(&self, relation: &Relation) -> Result<bool, Error> {
        let mut inner = self.inner.lock().map_err(Error::store_lock)?;
        if !inner.relations.remove(relation) {
            return Ok(false);
        }
        remove_indexed_relation(&mut inner.relations_by_from, &relation.from, relation);
        remove_indexed_relation(
            &mut inner.relations_by_type,
            &RelationTypeKey {
                from_type: relation.from.type_id,
                to_type: relation.to.type_id,
            },
            relation,
        );
        drop(inner);
        Ok(true)
    }

    pub(crate) fn has_relation_to_type<From, To>(&self, from_id: &ODataId) -> bool
    where
        From: 'static,
        To: 'static,
    {
        self.has_relation_from_to_type(
            &ResourceRef::of::<From>(from_id.clone()),
            TypeId::of::<To>(),
        )
    }

    pub(crate) fn has_relation_from_to_type(&self, from: &ResourceRef, to_type: TypeId) -> bool {
        let key = RelationTypeKey {
            from_type: from.type_id,
            to_type,
        };
        self.inner.lock().is_ok_and(|inner| {
            inner
                .relations_by_type
                .get(&key)
                .is_some_and(|relations| relations.iter().any(|relation| &relation.from == from))
        })
    }

    #[cfg(test)]
    pub(crate) fn query_members(&self, query_id: QueryId) -> Result<BTreeSet<ResourceRef>, Error> {
        self.inner
            .lock()
            .map(|inner| inner.by_query.get(&query_id).cloned().unwrap_or_default())
            .map_err(Error::store_lock)
    }

    #[cfg(test)]
    pub(crate) fn discovery_source_members(
        &self,
        source_id: DiscoverySourceId,
    ) -> Result<BTreeSet<ResourceRef>, Error> {
        self.inner
            .lock()
            .map(|inner| {
                inner
                    .by_discovery_source
                    .get(&source_id)
                    .cloned()
                    .unwrap_or_default()
            })
            .map_err(Error::store_lock)
    }

    fn typed_snapshot<T>(erased: &ErasedSnapshot) -> Option<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        let value = Arc::clone(&erased.value).downcast::<T>().ok()?;
        Some(ResourceSnapshot {
            id: erased.id.clone(),
            value,
            etag: erased.etag.clone(),
            fetched_at: erased.fetched_at,
            staleness: erased.staleness,
            observed_at: erased.observed_at,
        })
    }
}

#[derive(Default)]
struct ResourceStoreInner {
    resources: BTreeMap<ResourceKey, ErasedSnapshot>,
    by_type: BTreeMap<TypeId, BTreeSet<ODataId>>,
    by_query: BTreeMap<QueryId, BTreeSet<ResourceRef>>,
    by_discovery_source: BTreeMap<DiscoverySourceId, BTreeSet<ResourceRef>>,
    relations: BTreeSet<Relation>,
    relations_by_from: BTreeMap<ResourceRef, BTreeSet<Relation>>,
    relations_by_type: BTreeMap<RelationTypeKey, BTreeSet<Relation>>,
}

fn remove_indexed_relation<K>(
    index: &mut BTreeMap<K, BTreeSet<Relation>>,
    key: &K,
    relation: &Relation,
) where
    K: Ord + Clone,
{
    let Some(relations) = index.get_mut(key) else {
        return;
    };
    relations.remove(relation);
    if relations.is_empty() {
        index.remove(key);
    }
}
