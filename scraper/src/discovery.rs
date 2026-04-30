// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use nv_redfish::schema::log_service::LogService;
use nv_redfish::schema::sensor::Sensor;
use nv_redfish::schema::software_inventory::SoftwareInventory;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use serde::Deserialize;

use crate::DiscoveryError;
use crate::Error;
use crate::EventBus;
use crate::Lane;
use crate::RawResource;
use crate::Relation;
use crate::Scheduler;
use crate::ScraperEvent;

mod firmware_logs;
mod standard_sensors;

/// Discovery registration bundle.
///
/// Registering discovery is side-effect free. Discoverers run only when query
/// APIs demand candidates.
#[derive(Clone)]
pub struct Discovery {
    registration: DiscoveryRegistration,
}

impl Discovery {
    /// Returns the built-in standard discovery bundle.
    ///
    /// The bundle is registered for future discovery phases. Registration
    /// performs no discovery work and makes no BMC calls.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            registration: DiscoveryRegistration::Standard,
        }
    }

    /// Creates a manual typed discovery registration.
    #[must_use]
    pub fn manual<T, D>(discoverer: D) -> Self
    where
        T: Send + Sync + 'static,
        D: Discoverer<T>,
    {
        Self {
            registration: DiscoveryRegistration::Manual {
                type_id: TypeId::of::<T>(),
                discoverer: Arc::new(TypedDiscoverer::<T, D> {
                    discoverer,
                    resource_type: PhantomData,
                }),
            },
        }
    }
}

impl Debug for Discovery {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match &self.registration {
            DiscoveryRegistration::Standard => formatter.write_str("Discovery::standard"),
            DiscoveryRegistration::Manual { type_id, .. } => formatter
                .debug_struct("Discovery::manual")
                .field("type_id", type_id)
                .finish(),
        }
    }
}

#[derive(Clone)]
enum DiscoveryRegistration {
    Standard,
    Manual {
        type_id: TypeId,
        discoverer: Arc<dyn ErasedDiscoverer>,
    },
}

pub struct DiscoveryRegistry {
    next_source_id: u64,
    standard_source_id: Option<DiscoverySourceId>,
    discoverers: BTreeMap<TypeId, Vec<RegisteredDiscoverer>>,
}

impl Default for DiscoveryRegistry {
    fn default() -> Self {
        Self {
            next_source_id: 1,
            standard_source_id: None,
            discoverers: BTreeMap::new(),
        }
    }
}

impl Debug for DiscoveryRegistry {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("DiscoveryRegistry")
            .field("next_source_id", &self.next_source_id)
            .field("standard_source_id", &self.standard_source_id)
            .field("discoverer_types", &self.discoverers.len())
            .finish()
    }
}

impl DiscoveryRegistry {
    pub fn register(&mut self, discovery: Discovery) {
        match discovery.registration {
            DiscoveryRegistration::Standard => {
                if self.standard_source_id.is_none() {
                    self.standard_source_id = Some(self.allocate_source_id());
                }
            }
            DiscoveryRegistration::Manual {
                type_id,
                discoverer,
            } => {
                let source_id = self.allocate_source_id();
                self.discoverers
                    .entry(type_id)
                    .or_default()
                    .push(RegisteredDiscoverer {
                        source_id,
                        discoverer,
                    });
            }
        }
    }

    const fn allocate_source_id(&mut self) -> DiscoverySourceId {
        let source_id = DiscoverySourceId::new(self.next_source_id);
        self.next_source_id += 1;
        source_id
    }

    pub(crate) async fn discover<B, T>(
        &self,
        bmc: &B,
        scheduler: &Scheduler,
        events: &EventBus,
        hint: DiscoveryHint,
    ) -> Result<DiscoveryReport, Error>
    where
        B: Bmc,
        B::Error: 'static,
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let mut report = DiscoveryReport::default();
        if let Some(discoverers) = self.discoverers.get(&type_id) {
            let raw_fetcher = SchedulerRawFetcher {
                bmc,
                scheduler,
                events,
            };
            for registered in discoverers {
                let mut context = DiscoveryContext::new(
                    events,
                    registered.source_id,
                    type_id,
                    Some(&raw_fetcher),
                );
                let batch = registered
                    .discoverer
                    .discover(&mut context, hint.clone())
                    .await?;
                report.extend(registered.source_id, batch);
            }
        }
        if let Some(source_id) = self.standard_source_id {
            let context = TypedDiscoveryContext::new(events, bmc, scheduler);
            if type_id == TypeId::of::<Sensor>() {
                report.extend(
                    source_id,
                    standard_sensors::discover_sensors(&context).await?,
                );
            }
            if type_id == TypeId::of::<SoftwareInventory>() {
                report.extend(source_id, firmware_logs::discover_firmware(&context).await?);
            }
            if type_id == TypeId::of::<LogService>() {
                report.extend(
                    source_id,
                    firmware_logs::discover_log_services(&context).await?,
                );
            }
        }
        report.deduplicate();
        for source in &report.sources {
            if !source.ids.is_empty() {
                events.publish(ScraperEvent::Discovery(DiscoveryEvent::Candidates {
                    source_id: source.source_id,
                    type_id,
                    ids: source.ids.clone(),
                }));
            }
        }
        Ok(report)
    }
}

#[derive(Clone)]
struct RegisteredDiscoverer {
    source_id: DiscoverySourceId,
    discoverer: Arc<dyn ErasedDiscoverer>,
}

/// Manual discoverer for resources of type `T`.
pub trait Discoverer<T>: Send + Sync + 'static {
    /// Returns candidate resource ids for demand-driven query execution.
    fn discover<'a>(
        &'a self,
        cx: &'a mut DiscoveryContext<'a>,
        hint: DiscoveryHint,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoveryBatch, Error>> + Send + 'a>>;
}

trait ErasedDiscoverer: Send + Sync {
    fn discover<'a>(
        &'a self,
        cx: &'a mut DiscoveryContext<'a>,
        hint: DiscoveryHint,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoveryBatch, Error>> + Send + 'a>>;
}

struct TypedDiscoverer<T, D> {
    discoverer: D,
    resource_type: PhantomData<fn() -> T>,
}

impl<T, D> ErasedDiscoverer for TypedDiscoverer<T, D>
where
    T: Send + Sync + 'static,
    D: Discoverer<T>,
{
    fn discover<'a>(
        &'a self,
        cx: &'a mut DiscoveryContext<'a>,
        hint: DiscoveryHint,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoveryBatch, Error>> + Send + 'a>> {
        self.discoverer.discover(cx, hint)
    }
}

/// Stable identifier for a registered discovery source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoverySourceId(u64);

impl DiscoverySourceId {
    const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric identifier.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Discovery context passed to discoverers.
///
/// The context intentionally does not expose a raw BMC handle. Fetch helpers
/// route BMC I/O through the shared scraper scheduler.
pub struct DiscoveryContext<'a> {
    events: &'a EventBus,
    source_id: DiscoverySourceId,
    type_id: TypeId,
    raw_fetcher: Option<&'a dyn RawDiscoveryFetch>,
}

impl<'a> DiscoveryContext<'a> {
    const fn new(
        events: &'a EventBus,
        source_id: DiscoverySourceId,
        type_id: TypeId,
        raw_fetcher: Option<&'a dyn RawDiscoveryFetch>,
    ) -> Self {
        Self {
            events,
            source_id,
            type_id,
            raw_fetcher,
        }
    }

    /// Returns the discovery source currently running.
    #[must_use]
    pub const fn source_id(&self) -> DiscoverySourceId {
        self.source_id
    }

    /// Returns the resource type currently being discovered.
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Publishes a discovery event.
    pub fn publish(&self, event: DiscoveryEvent) {
        self.events.publish(ScraperEvent::Discovery(event));
    }

    /// Fetches an unknown or OEM Redfish resource through the scheduler.
    ///
    /// # Errors
    ///
    /// Returns an error when raw fetch support is unavailable, scheduler
    /// admission fails, or the BMC request fails.
    pub async fn fetch_raw(&self, id: impl Into<ODataId>) -> Result<Arc<RawResource>, Error> {
        let fetcher = self
            .raw_fetcher
            .ok_or(Error::Discovery(DiscoveryError::RawFetchUnavailable))?;
        fetcher.fetch_raw(id.into()).await
    }
}

trait RawDiscoveryFetch: Send + Sync {
    fn fetch_raw<'a>(
        &'a self,
        id: ODataId,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<RawResource>, Error>> + Send + 'a>>;
}

struct SchedulerRawFetcher<'a, B> {
    bmc: &'a B,
    scheduler: &'a Scheduler,
    events: &'a EventBus,
}

impl<B> RawDiscoveryFetch for SchedulerRawFetcher<'_, B>
where
    B: Bmc,
    B::Error: 'static,
{
    fn fetch_raw<'a>(
        &'a self,
        id: ODataId,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<RawResource>, Error>> + Send + 'a>> {
        Box::pin(async move {
            self.scheduler
                .get::<B, RawResource>(self.bmc, self.events, Lane::Discovery, id)
                .await
        })
    }
}

pub struct TypedDiscoveryContext<'a, B> {
    events: &'a EventBus,
    bmc: &'a B,
    scheduler: &'a Scheduler,
}

impl<'a, B> TypedDiscoveryContext<'a, B> {
    const fn new(events: &'a EventBus, bmc: &'a B, scheduler: &'a Scheduler) -> Self {
        Self {
            events,
            bmc,
            scheduler,
        }
    }
}

impl<B> TypedDiscoveryContext<'_, B>
where
    B: Bmc,
    B::Error: 'static,
{
    pub(super) async fn fetch<T>(&self, id: impl Into<ODataId>) -> Result<Arc<T>, Error>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        self.scheduler
            .get::<B, T>(self.bmc, self.events, Lane::Discovery, id.into())
            .await
    }
}

/// Discovery hints supplied to discoverers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryHint {
    id_contains: Vec<String>,
    relation_target_types: Vec<TypeId>,
    semantic: Vec<String>,
}

impl DiscoveryHint {
    /// Creates an id-substring discovery hint.
    #[must_use]
    pub fn id_contains(needle: impl Into<String>) -> Self {
        Self {
            id_contains: vec![needle.into()],
            relation_target_types: Vec::new(),
            semantic: Vec::new(),
        }
    }

    /// Creates a relation-constrained discovery hint.
    #[must_use]
    pub fn related_to<T>() -> Self
    where
        T: 'static,
    {
        Self::related_to_type(TypeId::of::<T>())
    }

    /// Creates a relation-constrained discovery hint for a target type id.
    #[must_use]
    pub fn related_to_type(type_id: TypeId) -> Self {
        Self {
            id_contains: Vec::new(),
            relation_target_types: vec![type_id],
            semantic: Vec::new(),
        }
    }

    /// Creates a semantic discovery hint.
    #[must_use]
    pub fn semantic(value: impl Into<String>) -> Self {
        Self {
            id_contains: Vec::new(),
            relation_target_types: Vec::new(),
            semantic: vec![value.into()],
        }
    }

    /// Returns id-substring hints.
    #[must_use]
    pub fn id_contains_hints(&self) -> &[String] {
        &self.id_contains
    }

    /// Returns target types from relation-constrained hints.
    #[must_use]
    pub fn relation_target_types(&self) -> &[TypeId] {
        &self.relation_target_types
    }

    /// Returns semantic hints.
    #[must_use]
    pub fn semantic_hints(&self) -> &[String] {
        &self.semantic
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.id_contains.extend(other.id_contains);
        self.relation_target_types
            .extend(other.relation_target_types);
        self.semantic.extend(other.semantic);
        self.deduplicate();
    }

    fn deduplicate(&mut self) {
        self.id_contains = self
            .id_contains
            .drain(..)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.relation_target_types = self
            .relation_target_types
            .drain(..)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.semantic = self
            .semantic
            .drain(..)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
}

/// Batch of discovery output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryBatch {
    /// Candidate resource ids.
    pub candidates: Vec<ODataId>,
    /// Relations learned while discovering candidates.
    pub relations: Vec<Relation>,
}

impl DiscoveryBatch {
    /// Creates a batch from candidate resource ids.
    #[must_use]
    pub fn candidates(candidates: impl IntoIterator<Item = ODataId>) -> Self {
        Self {
            candidates: candidates.into_iter().collect(),
            relations: Vec::new(),
        }
    }

    /// Adds relations to this discovery batch.
    #[must_use]
    pub fn with_relations(mut self, relations: impl IntoIterator<Item = Relation>) -> Self {
        self.relations.extend(relations);
        self
    }

    fn extend(&mut self, other: Self) {
        self.candidates.extend(other.candidates);
        self.relations.extend(other.relations);
    }

    fn deduplicate(&mut self) {
        self.candidates = self
            .candidates
            .drain(..)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.relations = self
            .relations
            .drain(..)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
}

#[derive(Default)]
pub struct DiscoveryReport {
    pub(crate) batch: DiscoveryBatch,
    pub(crate) sources: Vec<DiscoverySourceCandidates>,
}

impl DiscoveryReport {
    fn extend(&mut self, source_id: DiscoverySourceId, batch: DiscoveryBatch) {
        self.sources.push(DiscoverySourceCandidates {
            source_id,
            ids: batch.candidates.clone(),
        });
        self.batch.extend(batch);
    }

    fn deduplicate(&mut self) {
        self.batch.deduplicate();
        for source in &mut self.sources {
            source.ids = source
                .ids
                .drain(..)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        }
        self.sources.retain(|source| !source.ids.is_empty());
    }
}

pub struct DiscoverySourceCandidates {
    pub(crate) source_id: DiscoverySourceId,
    pub(crate) ids: Vec<ODataId>,
}

/// Discovery event emitted by the scraper.
#[derive(Clone, Debug)]
pub enum DiscoveryEvent {
    /// Candidate resource ids were discovered for a resource type.
    Candidates {
        /// Discovery source that produced these candidates.
        source_id: DiscoverySourceId,
        /// Rust type id for the resource type.
        type_id: TypeId,
        /// Candidate resource ids.
        ids: Vec<ODataId>,
    },
}
