// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::any::TypeId;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::store::InsertStatus;
use crate::Error;
use crate::Lane;
use crate::ResourceEvent;
use crate::ResourceSnapshot;
use crate::SchedulerError;
use crate::Scraper;
use crate::ScraperEvent;
use crate::StoreError;

/// Direct typed resource client.
///
/// Refreshes issued through this client perform BMC I/O through the scheduler.
/// Cached reads inspect the local store without BMC I/O.
#[derive(Debug)]
pub struct ResourceClient<B, T> {
    scraper: Scraper<B>,
    resource_type: PhantomData<fn() -> T>,
}

impl<B, T> Clone for ResourceClient<B, T> {
    fn clone(&self) -> Self {
        Self {
            scraper: self.scraper.clone(),
            resource_type: PhantomData,
        }
    }
}

impl<B, T> ResourceClient<B, T> {
    pub(crate) const fn new(scraper: Scraper<B>) -> Self {
        Self {
            scraper,
            resource_type: PhantomData,
        }
    }

    /// Returns the local snapshot for `id` without scheduling BMC I/O.
    ///
    /// Returns `None` when the resource has not been refreshed into the local
    /// store for this resource type.
    #[must_use]
    pub fn cached(&self, id: impl Into<ODataId>) -> Option<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        self.scraper.inner().store.get::<T>(&id.into())
    }

    /// Returns the local snapshot and reports it against `freshness`.
    ///
    /// This is still a local cached read and never schedules BMC I/O. The
    /// returned snapshot is marked stale when the cached value is older than
    /// the requested freshness.
    #[must_use]
    pub fn cached_with_freshness(
        &self,
        id: impl Into<ODataId>,
        freshness: Duration,
    ) -> Option<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        self.cached(id)
            .map(|snapshot| snapshot.with_desired_freshness(Some(freshness)))
    }

    /// Lists local snapshots for this resource type without scheduling BMC I/O.
    ///
    /// The result includes only snapshots already present in the local store.
    #[must_use]
    pub fn list_cached(&self) -> Vec<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        self.scraper.inner().store.list::<T>()
    }

    /// Lists local snapshots and reports them against `freshness`.
    ///
    /// This is still a local cached read and never schedules BMC I/O.
    #[must_use]
    pub fn list_cached_with_freshness(&self, freshness: Duration) -> Vec<ResourceSnapshot<T>>
    where
        T: Send + Sync + 'static,
    {
        self.list_cached()
            .into_iter()
            .map(|snapshot| snapshot.with_desired_freshness(Some(freshness)))
            .collect()
    }

    /// Refreshes a known resource URI through the scraper scheduler.
    ///
    /// This performs BMC I/O, stores the resulting snapshot, emits a resource
    /// event, and returns the accepted snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the scheduler cannot record work, the BMC request
    /// fails, or the fetched snapshot cannot be inserted into the store.
    pub async fn refresh(&self, id: impl Into<ODataId>) -> Result<ResourceSnapshot<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        self.refresh_with_lane(id, Lane::Interactive).await
    }

    pub(crate) async fn refresh_with_lane(
        &self,
        id: impl Into<ODataId>,
        lane: Lane,
    ) -> Result<ResourceSnapshot<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let id = id.into();
        let key = OperationKey::get::<T>(id.clone());
        match self.scraper.inner().refreshes.join_or_start(key)? {
            RefreshTicket::Owner(completion) => self.refresh_as_owner(id, lane, completion).await,
            RefreshTicket::Waiter(receiver) => await_refresh::<T>(receiver).await,
        }
    }

    async fn refresh_as_owner(
        &self,
        id: ODataId,
        lane: Lane,
        completion: RefreshCompletion<'_>,
    ) -> Result<ResourceSnapshot<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let result = match self.fetch_snapshot(id.clone(), lane).await {
            Ok(snapshot) => self.store_and_publish(snapshot),
            Err(error) => {
                self.publish_error(id, &error);
                Err(error)
            }
        };
        completion.complete(result.clone().map(ErasedRefresh::from_snapshot));
        result
    }

    async fn fetch_snapshot(&self, id: ODataId, lane: Lane) -> Result<ResourceSnapshot<T>, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let value = self
            .scraper
            .inner()
            .scheduler
            .get::<B, T>(
                &self.scraper.inner().bmc,
                &self.scraper.inner().events,
                lane,
                id.clone(),
            )
            .await?;
        let etag = value.etag().cloned();
        Ok(ResourceSnapshot::new_fresh(id, value, etag))
    }

    fn publish_error(&self, id: ODataId, error: &Error)
    where
        T: 'static,
    {
        self.scraper
            .inner()
            .events
            .publish(ScraperEvent::Resource(ResourceEvent::Error {
                type_id: TypeId::of::<T>(),
                id,
                error: Arc::new(error.clone()),
            }));
    }

    fn store_and_publish(&self, snapshot: ResourceSnapshot<T>) -> Result<ResourceSnapshot<T>, Error>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = snapshot.id.clone();
        let status = self.scraper.inner().store.insert(snapshot.clone())?;
        let event = match status {
            InsertStatus::Added => ResourceEvent::Added { type_id, id },
            InsertStatus::Updated => ResourceEvent::Updated { type_id, id },
        };
        self.scraper
            .inner()
            .events
            .publish(ScraperEvent::Resource(event));
        Ok(snapshot)
    }
}

type SharedRefresh = Option<Result<ErasedRefresh, Arc<Error>>>;

#[derive(Debug, Default)]
pub struct RefreshCoalescer {
    in_flight: Mutex<BTreeMap<OperationKey, watch::Receiver<SharedRefresh>>>,
}

impl RefreshCoalescer {
    fn join_or_start(&self, key: OperationKey) -> Result<RefreshTicket<'_>, Error> {
        let mut in_flight = self.in_flight.lock().map_err(Error::scheduler_lock)?;
        if let Some(receiver) = in_flight.get(&key) {
            return Ok(RefreshTicket::Waiter(receiver.clone()));
        }
        let (sender, receiver) = watch::channel(None);
        in_flight.insert(key.clone(), receiver);
        drop(in_flight);
        Ok(RefreshTicket::Owner(RefreshCompletion {
            key,
            sender,
            in_flight: &self.in_flight,
            completed: false,
        }))
    }
}

enum RefreshTicket<'a> {
    Owner(RefreshCompletion<'a>),
    Waiter(watch::Receiver<SharedRefresh>),
}

struct RefreshCompletion<'a> {
    key: OperationKey,
    sender: watch::Sender<SharedRefresh>,
    in_flight: &'a Mutex<BTreeMap<OperationKey, watch::Receiver<SharedRefresh>>>,
    completed: bool,
}

impl RefreshCompletion<'_> {
    fn complete(mut self, result: Result<ErasedRefresh, Error>) {
        let shared = result.map_err(Arc::new);
        self.sender.send_replace(Some(shared));
        self.remove_inflight();
        self.completed = true;
    }

    fn remove_inflight(&self) {
        self.in_flight
            .lock()
            .map(|mut in_flight| in_flight.remove(&self.key))
            .ok();
    }
}

impl Drop for RefreshCompletion<'_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let error = Error::Scheduler(SchedulerError::CoalescedOwnerCancelled);
        self.sender.send_replace(Some(Err(Arc::new(error))));
        self.remove_inflight();
    }
}

async fn await_refresh<T>(
    mut receiver: watch::Receiver<SharedRefresh>,
) -> Result<ResourceSnapshot<T>, Error>
where
    T: Send + Sync + 'static,
{
    loop {
        let current = receiver.borrow().clone();
        match current {
            Some(Ok(snapshot)) => return snapshot.into_snapshot::<T>(),
            Some(Err(error)) => return Err((*error).clone()),
            None => receiver.changed().await.map_err(|error| {
                Error::Scheduler(SchedulerError::AdmissionClosed(error.to_string()))
            })?,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OperationKey {
    operation: OperationKind,
    type_id: TypeId,
    id: ODataId,
}

impl OperationKey {
    const fn get<T>(id: ODataId) -> Self
    where
        T: 'static,
    {
        Self {
            operation: OperationKind::Get,
            type_id: TypeId::of::<T>(),
            id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OperationKind {
    Get,
}

#[derive(Clone, Debug)]
struct ErasedRefresh {
    id: ODataId,
    value: Arc<dyn Any + Send + Sync>,
    etag: Option<ODataETag>,
    fetched_at: SystemTime,
    observed_at: Instant,
    staleness: crate::Staleness,
}

impl ErasedRefresh {
    fn from_snapshot<T>(snapshot: ResourceSnapshot<T>) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            id: snapshot.id,
            value: snapshot.value,
            etag: snapshot.etag,
            fetched_at: snapshot.fetched_at,
            observed_at: snapshot.observed_at,
            staleness: snapshot.staleness,
        }
    }

    fn into_snapshot<T>(self) -> Result<ResourceSnapshot<T>, Error>
    where
        T: Send + Sync + 'static,
    {
        let value = self
            .value
            .downcast::<T>()
            .map_err(|_| Error::Store(StoreError::CoalescedRefreshTypeMismatch))?;
        Ok(ResourceSnapshot {
            id: self.id,
            value,
            etag: self.etag,
            fetched_at: self.fetched_at,
            observed_at: self.observed_at,
            staleness: self.staleness,
        })
    }
}
