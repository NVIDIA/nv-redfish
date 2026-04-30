use super::BmcCapacity;
use super::Discoverer;
use super::Discovery;
use super::DiscoveryBatch;
use super::DiscoveryContext;
use super::DiscoveryEvent;
use super::DiscoveryHint;
use super::EventEnvelope;
use super::Lane;
use super::LoadState;
use super::Predicate;
use super::PredicateContext;
use super::QueryEvent;
use super::QueryKind;
use super::RawResource;
use super::Relation;
use super::RelationKind;
use super::ResourceEvent;
use super::ResourceRef;
use super::SchedulerEvent;
use super::Scraper;
use super::ScraperEvent;
use super::Staleness;
use super::TypedResourceEvent;
use crate::predicate::resource as resource_predicate;
use crate::predicate::sensor as sensor_predicate;
use crate::scheduler::Operation;
use nv_redfish::schema::log_service::LogService;
use nv_redfish::schema::physical_context::PhysicalContext;
use nv_redfish::schema::sensor::ReadingType;
use nv_redfish::schema::sensor::Sensor as RedfishSensor;
use nv_redfish::schema::software_inventory::SoftwareInventory;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Action;
use nv_redfish_core::BoxTryStream;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::FilterQuery;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::any::TypeId;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::Notify;
use tokio::task::yield_now;
use tokio::time::advance;

#[derive(Clone, Debug, Default)]
struct RecordingBmc {
    state: Arc<Mutex<FakeState>>,
}

impl RecordingBmc {
    fn insert<T>(&self, resource: &T) -> Result<(), Box<dyn StdError>>
    where
        T: EntityTypeRef + Serialize,
    {
        let value = serde_json::to_value(resource)?;
        self.state
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .responses
            .insert(resource.odata_id().clone(), Ok(value));
        Ok(())
    }

    fn insert_value(&self, id: ODataId, value: Value) -> Result<(), Box<dyn StdError>> {
        self.state
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .responses
            .insert(id, Ok(value));
        Ok(())
    }

    fn fail(&self, id: ODataId, error: FakeBmcError) -> Result<(), Box<dyn StdError>> {
        self.state
            .lock()
            .map_err(|lock_error| TestFailure::boxed(lock_error.to_string()))?
            .responses
            .insert(id, Err(error));
        Ok(())
    }

    fn request_count(&self) -> Result<usize, Box<dyn StdError>> {
        Ok(self
            .state
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .requested_ids
            .len())
    }

    fn requested_ids(&self) -> Result<Vec<ODataId>, Box<dyn StdError>> {
        Ok(self
            .state
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .requested_ids
            .clone())
    }
}

#[derive(Clone, Debug, Default)]
struct FakeState {
    responses: BTreeMap<ODataId, Result<Value, FakeBmcError>>,
    requested_ids: Vec<ODataId>,
}

#[derive(Clone, Debug, Default)]
struct BlockingBmc {
    responses: Arc<Mutex<BTreeMap<ODataId, Result<Value, FakeBmcError>>>>,
    stats: Arc<Mutex<BlockingStats>>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    released: Arc<AtomicBool>,
}

impl BlockingBmc {
    fn insert<T>(&self, resource: &T) -> Result<(), Box<dyn StdError>>
    where
        T: EntityTypeRef + Serialize,
    {
        let value = serde_json::to_value(resource)?;
        self.responses
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .insert(resource.odata_id().clone(), Ok(value));
        Ok(())
    }

    fn fail(&self, id: ODataId, error: FakeBmcError) -> Result<(), Box<dyn StdError>> {
        self.responses
            .lock()
            .map_err(|lock_error| TestFailure::boxed(lock_error.to_string()))?
            .insert(id, Err(error));
        Ok(())
    }

    async fn wait_for_in_flight(&self, expected: usize) -> Result<(), Box<dyn StdError>> {
        loop {
            if self.current_in_flight()? >= expected {
                return Ok(());
            }
            self.entered.notified().await;
        }
    }

    fn release_all(&self) {
        self.released.store(true, Ordering::SeqCst);
        self.release.notify_waiters();
    }

    fn block_all(&self) {
        self.released.store(false, Ordering::SeqCst);
    }

    fn current_in_flight(&self) -> Result<usize, Box<dyn StdError>> {
        Ok(self
            .stats
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .current_in_flight)
    }

    fn max_in_flight(&self) -> Result<usize, Box<dyn StdError>> {
        Ok(self
            .stats
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .max_in_flight)
    }

    fn request_count(&self) -> Result<usize, Box<dyn StdError>> {
        Ok(self
            .stats
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .request_count)
    }
}

#[derive(Clone, Debug, Default)]
struct BlockingStats {
    current_in_flight: usize,
    max_in_flight: usize,
    request_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FakeBmcError {
    BadJson(String),
    Missing(ODataId),
    NotSupported,
    Response(String),
}

impl Display for FakeBmcError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::BadJson(error) => write!(formatter, "bad json: {error}"),
            Self::Missing(id) => write!(formatter, "missing response for {id}"),
            Self::NotSupported => formatter.write_str("not supported"),
            Self::Response(error) => write!(formatter, "response: {error}"),
        }
    }
}

impl StdError for FakeBmcError {}

impl nv_redfish_core::Bmc for RecordingBmc {
    type Error = FakeBmcError;

    async fn expand<T>(&self, _id: &ODataId, _query: ExpandQuery) -> Result<Arc<T>, Self::Error>
    where
        T: Expandable,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn get<T>(&self, id: &ODataId) -> Result<Arc<T>, Self::Error>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        let response = {
            let mut state = self
                .state
                .lock()
                .map_err(|error| FakeBmcError::Response(error.to_string()))?;
            state.requested_ids.push(id.clone());
            state
                .responses
                .get(id)
                .cloned()
                .ok_or_else(|| FakeBmcError::Missing(id.clone()))?
        }?;
        serde_json::from_value(response)
            .map(Arc::new)
            .map_err(|error| FakeBmcError::BadJson(error.to_string()))
    }

    async fn filter<T>(&self, _id: &ODataId, _query: FilterQuery) -> Result<Arc<T>, Self::Error>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn create<V, R>(
        &self,
        _id: &ODataId,
        _query: &V,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        V: Send + Sync + Serialize,
        R: Send + Sync + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn update<V, R>(
        &self,
        _id: &ODataId,
        _etag: Option<&ODataETag>,
        _update: &V,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        V: Send + Sync + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn delete<R>(&self, _id: &ODataId) -> Result<ModificationResponse<R>, Self::Error>
    where
        R: EntityTypeRef + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn action<T, R>(
        &self,
        _action: &Action<T, R>,
        _params: &T,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        T: Send + Sync + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn stream<T>(&self, _uri: &str) -> Result<BoxTryStream<T, Self::Error>, Self::Error>
    where
        T: Sized + for<'de> Deserialize<'de> + Send + 'static,
    {
        Err(FakeBmcError::NotSupported)
    }
}

impl nv_redfish_core::Bmc for BlockingBmc {
    type Error = FakeBmcError;

    async fn expand<T>(&self, _id: &ODataId, _query: ExpandQuery) -> Result<Arc<T>, Self::Error>
    where
        T: Expandable,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn get<T>(&self, id: &ODataId) -> Result<Arc<T>, Self::Error>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        {
            let mut stats = self
                .stats
                .lock()
                .map_err(|error| FakeBmcError::Response(error.to_string()))?;
            stats.current_in_flight += 1;
            stats.request_count += 1;
            stats.max_in_flight = stats.max_in_flight.max(stats.current_in_flight);
        }
        self.entered.notify_waiters();

        while !self.released.load(Ordering::SeqCst) {
            self.release.notified().await;
        }

        {
            let mut stats = self
                .stats
                .lock()
                .map_err(|error| FakeBmcError::Response(error.to_string()))?;
            stats.current_in_flight = stats.current_in_flight.saturating_sub(1);
        }

        let response = self
            .responses
            .lock()
            .map_err(|error| FakeBmcError::Response(error.to_string()))?
            .get(id)
            .cloned()
            .ok_or_else(|| FakeBmcError::Missing(id.clone()))??;
        serde_json::from_value(response)
            .map(Arc::new)
            .map_err(|error| FakeBmcError::BadJson(error.to_string()))
    }

    async fn filter<T>(&self, _id: &ODataId, _query: FilterQuery) -> Result<Arc<T>, Self::Error>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de> + 'static,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn create<V, R>(
        &self,
        _id: &ODataId,
        _query: &V,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        V: Send + Sync + Serialize,
        R: Send + Sync + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn update<V, R>(
        &self,
        _id: &ODataId,
        _etag: Option<&ODataETag>,
        _update: &V,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        V: Send + Sync + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn delete<R>(&self, _id: &ODataId) -> Result<ModificationResponse<R>, Self::Error>
    where
        R: EntityTypeRef + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn action<T, R>(
        &self,
        _action: &Action<T, R>,
        _params: &T,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        T: Send + Sync + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    {
        Err(FakeBmcError::NotSupported)
    }

    async fn stream<T>(&self, _uri: &str) -> Result<BoxTryStream<T, Self::Error>, Self::Error>
    where
        T: Sized + for<'de> Deserialize<'de> + Send + 'static,
    {
        Err(FakeBmcError::NotSupported)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TestResource {
    #[serde(rename = "@odata.id")]
    id: ODataId,
    #[serde(
        rename = "@odata.etag",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    etag: Option<ODataETag>,
    name: String,
}

impl EntityTypeRef for TestResource {
    fn odata_id(&self) -> &ODataId {
        &self.id
    }

    fn etag(&self) -> Option<&ODataETag> {
        self.etag.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OtherResource {
    #[serde(rename = "@odata.id")]
    id: ODataId,
    #[serde(
        rename = "@odata.etag",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    etag: Option<ODataETag>,
    name: String,
}

impl EntityTypeRef for OtherResource {
    fn odata_id(&self) -> &ODataId {
        &self.id
    }

    fn etag(&self) -> Option<&ODataETag> {
        self.etag.as_ref()
    }
}

#[derive(Debug)]
struct Sensor;

#[derive(Clone, Debug)]
struct RawDiscoverer {
    root: ODataId,
}

impl RawDiscoverer {
    fn new(root: &str) -> Self {
        Self { root: id(root) }
    }
}

impl Discoverer<TestResource> for RawDiscoverer {
    fn discover<'a>(
        &'a self,
        cx: &'a mut DiscoveryContext<'a>,
        _hint: DiscoveryHint,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoveryBatch, super::Error>> + Send + 'a>> {
        Box::pin(async move {
            let raw = cx.fetch_raw(self.root.clone()).await?;
            let candidates = raw
                .value()
                .get("Members")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|member| member.get("@odata.id").and_then(Value::as_str))
                .map(|id| ODataId::from(id.to_owned()));
            Ok(DiscoveryBatch::candidates(candidates))
        })
    }
}

#[derive(Clone, Debug)]
struct TestDiscoverer {
    candidates: Vec<ODataId>,
    hints: Arc<Mutex<Vec<DiscoveryHint>>>,
    invoked: Arc<AtomicUsize>,
    relations: Vec<Relation>,
}

impl TestDiscoverer {
    fn new(candidates: Vec<ODataId>) -> Self {
        Self {
            candidates,
            hints: Arc::new(Mutex::new(Vec::new())),
            invoked: Arc::new(AtomicUsize::new(0)),
            relations: Vec::new(),
        }
    }

    fn with_relations(mut self, relations: Vec<Relation>) -> Self {
        self.relations = relations;
        self
    }

    fn invoked_count(&self) -> usize {
        self.invoked.load(Ordering::Relaxed)
    }

    fn hints(&self) -> Result<Vec<DiscoveryHint>, Box<dyn StdError>> {
        Ok(self
            .hints
            .lock()
            .map_err(|error| TestFailure::boxed(error.to_string()))?
            .clone())
    }
}

impl Discoverer<TestResource> for TestDiscoverer {
    fn discover<'a>(
        &'a self,
        _cx: &'a mut DiscoveryContext<'a>,
        hint: DiscoveryHint,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoveryBatch, super::Error>> + Send + 'a>> {
        let candidates = self.candidates.clone();
        let relations = self.relations.clone();
        self.invoked.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut hints) = self.hints.lock() {
            hints.push(hint);
        }
        Box::pin(
            async move { Ok(DiscoveryBatch::candidates(candidates).with_relations(relations)) },
        )
    }
}

#[derive(Clone, Debug)]
struct NameContainsPredicate {
    needle: String,
    hint: Option<DiscoveryHint>,
}

impl NameContainsPredicate {
    fn new(needle: &str) -> Self {
        Self {
            needle: needle.to_owned(),
            hint: None,
        }
    }

    fn with_hint(needle: &str, hint: DiscoveryHint) -> Self {
        Self {
            needle: needle.to_owned(),
            hint: Some(hint),
        }
    }
}

impl Predicate<TestResource> for NameContainsPredicate {
    fn candidate_hint(&self) -> Option<DiscoveryHint> {
        self.hint.clone()
    }

    fn matches_snapshot(
        &self,
        snapshot: &super::ResourceSnapshot<TestResource>,
        _context: &PredicateContext<'_>,
    ) -> bool {
        snapshot.value.name.contains(&self.needle)
    }
}

#[derive(Debug)]
struct TestFailure {
    message: String,
}

impl TestFailure {
    fn boxed(message: String) -> Box<dyn StdError> {
        Box::new(Self { message })
    }
}

impl Display for TestFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.message)
    }
}

impl StdError for TestFailure {}

fn id(value: &str) -> ODataId {
    value.to_owned().into()
}

fn etag(value: &str) -> ODataETag {
    value.to_owned().into()
}

fn resource(path: &str, name: &str) -> TestResource {
    TestResource {
        id: id(path),
        etag: Some(etag("etag")),
        name: name.to_owned(),
    }
}

fn other_resource(path: &str, name: &str) -> OtherResource {
    OtherResource {
        id: id(path),
        etag: Some(etag("other-etag")),
        name: name.to_owned(),
    }
}

fn relation(from: &str, to: &str) -> Relation {
    Relation::new(
        ResourceRef::of::<TestResource>(id(from)),
        ResourceRef::of::<OtherResource>(id(to)),
        RelationKind::RelatedTo,
    )
}

fn redfish_resource(path: &str, name: &str) -> Value {
    let id = path.rsplit('/').next().map_or(path, |id| id);
    json!({
        "@odata.id": path,
        "@odata.etag": null,
        "@Redfish.Settings": null,
        "@Redfish.SettingsApplyTime": null,
        "Id": id,
        "Name": name,
    })
}

fn service_root(chassis_collection: &str) -> Value {
    let mut root = redfish_resource("/redfish/v1", "Service Root");
    root["Chassis"] = json!({ "@odata.id": chassis_collection });
    root["Links"] = json!({
        "Sessions": { "@odata.id": "/redfish/v1/SessionService/Sessions" }
    });
    root
}

fn service_root_with_links(
    chassis_collection: Option<&str>,
    systems_collection: Option<&str>,
    managers_collection: Option<&str>,
    update_service_path: Option<&str>,
) -> Value {
    let mut root = redfish_resource("/redfish/v1", "Service Root");
    if let Some(chassis_collection) = chassis_collection {
        root["Chassis"] = json!({ "@odata.id": chassis_collection });
    }
    if let Some(systems_collection) = systems_collection {
        root["Systems"] = json!({ "@odata.id": systems_collection });
    }
    if let Some(managers_collection) = managers_collection {
        root["Managers"] = json!({ "@odata.id": managers_collection });
    }
    if let Some(update_service_path) = update_service_path {
        root["UpdateService"] = json!({ "@odata.id": update_service_path });
    }
    root["Links"] = json!({
        "Sessions": { "@odata.id": "/redfish/v1/SessionService/Sessions" }
    });
    root
}

fn chassis_collection(path: &str, chassis_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Chassis Collection");
    collection["Members"] = json!(chassis_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn system_collection(path: &str, system_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Computer System Collection");
    collection["Members"] = json!(system_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn manager_collection(path: &str, manager_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Manager Collection");
    collection["Members"] = json!(manager_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn sensor_collection(path: &str, sensor_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Sensor Collection");
    collection["Members"] = json!(sensor_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn redfish_collection(path: &str, name: &str) -> Value {
    json!({
        "@odata.type": "#ResourceCollection.ResourceCollection",
        "@odata.id": path,
        "@odata.etag": null,
        "@Redfish.Settings": null,
        "@Redfish.SettingsApplyTime": null,
        "Name": name,
    })
}

fn chassis(
    path: &str,
    sensors_path: Option<&str>,
    environment_metrics_path: Option<&str>,
) -> Value {
    let mut chassis = redfish_resource(path, "Chassis");
    chassis["ChassisType"] = json!("RackMount");
    if let Some(sensors_path) = sensors_path {
        chassis["Sensors"] = json!({ "@odata.id": sensors_path });
    }
    if let Some(environment_metrics_path) = environment_metrics_path {
        chassis["EnvironmentMetrics"] = json!({ "@odata.id": environment_metrics_path });
    }
    chassis
}

fn chassis_with_log_services(path: &str, log_services_path: &str) -> Value {
    let mut chassis = chassis(path, None, None);
    chassis["LogServices"] = json!({ "@odata.id": log_services_path });
    chassis
}

fn computer_system(path: &str, log_services_path: Option<&str>) -> Value {
    let mut system = redfish_resource(path, "Computer System");
    system["SystemType"] = json!("Physical");
    if let Some(log_services_path) = log_services_path {
        system["LogServices"] = json!({ "@odata.id": log_services_path });
    }
    system
}

fn manager(path: &str, log_services_path: Option<&str>) -> Value {
    let mut manager = redfish_resource(path, "Manager");
    manager["ManagerType"] = json!("BMC");
    if let Some(log_services_path) = log_services_path {
        manager["LogServices"] = json!({ "@odata.id": log_services_path });
    }
    manager
}

fn update_service(
    path: &str,
    firmware_inventory_path: Option<&str>,
    software_inventory_path: Option<&str>,
) -> Value {
    let mut update_service = redfish_resource(path, "Update Service");
    if let Some(firmware_inventory_path) = firmware_inventory_path {
        update_service["FirmwareInventory"] = json!({ "@odata.id": firmware_inventory_path });
    }
    if let Some(software_inventory_path) = software_inventory_path {
        update_service["SoftwareInventory"] = json!({ "@odata.id": software_inventory_path });
    }
    update_service
}

fn software_inventory_collection(path: &str, inventory_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Software Inventory Collection");
    collection["Members"] = json!(inventory_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn software_inventory(path: &str, name: &str, version: &str) -> Value {
    let mut inventory = redfish_resource(path, name);
    inventory["Version"] = json!(version);
    inventory
}

fn log_service_collection(path: &str, log_service_ids: &[&str]) -> Value {
    let mut collection = redfish_collection(path, "Log Service Collection");
    collection["Members"] = json!(log_service_ids
        .iter()
        .map(|id| json!({ "@odata.id": id }))
        .collect::<Vec<_>>());
    collection
}

fn log_service(path: &str, name: &str) -> Value {
    let mut service = redfish_resource(path, name);
    service["OverWritePolicy"] = json!("WrapsWhenFull");
    service
}

fn environment_metrics(path: &str, sensor_id: &str) -> Value {
    let mut metrics = redfish_resource(path, "Environment Metrics");
    metrics["TemperatureCelsius"] = json!({ "DataSourceUri": sensor_id });
    metrics
}

fn redfish_sensor(path: &str, name: &str) -> Value {
    let mut sensor = redfish_resource(path, name);
    sensor["ReadingType"] = json!("Temperature");
    sensor
}

fn redfish_sensor_with_context(path: &str, name: &str, context: &str) -> Value {
    let mut sensor = redfish_sensor(path, name);
    sensor["PhysicalContext"] = json!(context);
    sensor
}

fn insert_standard_sensor_root(
    bmc: &RecordingBmc,
    chassis_value: Value,
    sensor_collection_value: Option<Value>,
    environment_metrics_value: Option<Value>,
    sensors: &[(&str, &str)],
) -> Result<(), Box<dyn StdError>> {
    bmc.insert_value(id("/redfish/v1"), service_root("/redfish/v1/Chassis"))?;
    bmc.insert_value(
        id("/redfish/v1/Chassis"),
        chassis_collection("/redfish/v1/Chassis", &["/redfish/v1/Chassis/1"]),
    )?;
    bmc.insert_value(id("/redfish/v1/Chassis/1"), chassis_value)?;
    if let Some(sensor_collection_value) = sensor_collection_value {
        bmc.insert_value(id("/redfish/v1/Chassis/1/Sensors"), sensor_collection_value)?;
    }
    if let Some(environment_metrics_value) = environment_metrics_value {
        bmc.insert_value(
            id("/redfish/v1/Chassis/1/EnvironmentMetrics"),
            environment_metrics_value,
        )?;
    }
    for (path, name) in sensors {
        bmc.insert_value(id(path), redfish_sensor(path, name))?;
    }
    Ok(())
}

fn scheduler_work_count(scraper: &Scraper<RecordingBmc>) -> Result<usize, Box<dyn StdError>> {
    Ok(scraper.inner().scheduler.records()?.len())
}

fn next_resource_event(
    events: &mut super::EventReceiver,
) -> Result<EventEnvelope, Box<dyn StdError>> {
    loop {
        let envelope = tokio_test::block_on(events.recv())?;
        if matches!(envelope.event, ScraperEvent::Resource(_)) {
            return Ok(envelope);
        }
    }
}

fn next_scheduler_event(
    events: &mut super::EventReceiver,
) -> Result<EventEnvelope, Box<dyn StdError>> {
    loop {
        let envelope = tokio_test::block_on(events.recv())?;
        if matches!(envelope.event, ScraperEvent::Scheduler(_)) {
            return Ok(envelope);
        }
    }
}

fn next_discovery_event(
    events: &mut super::EventReceiver,
) -> Result<EventEnvelope, Box<dyn StdError>> {
    loop {
        let envelope = tokio_test::block_on(events.recv())?;
        if matches!(envelope.event, ScraperEvent::Discovery(_)) {
            return Ok(envelope);
        }
    }
}

fn next_query_event(events: &mut super::EventReceiver) -> Result<EventEnvelope, Box<dyn StdError>> {
    loop {
        let envelope = tokio_test::block_on(events.recv())?;
        if matches!(envelope.event, ScraperEvent::Query(_)) {
            return Ok(envelope);
        }
    }
}

fn drain_resource_events(
    events: &mut super::EventReceiver,
) -> Result<Vec<ResourceEvent>, Box<dyn StdError>> {
    let mut resources = Vec::new();
    loop {
        match events.try_recv() {
            Ok(envelope) => {
                if let ScraperEvent::Resource(event) = envelope.event {
                    resources.push(event);
                }
            }
            Err(TryRecvError::Empty) => return Ok(resources),
            Err(TryRecvError::Closed) => {
                return Err(TestFailure::boxed(String::from("event stream closed")));
            }
            Err(TryRecvError::Lagged(count)) => {
                return Err(TestFailure::boxed(format!(
                    "event stream lagged by {count}"
                )));
            }
        }
    }
}

fn drain_scheduler_stats(
    events: &mut super::EventReceiver,
) -> Result<Vec<super::SchedulerStats>, Box<dyn StdError>> {
    let mut stats = Vec::new();
    loop {
        match events.try_recv() {
            Ok(envelope) => {
                if let ScraperEvent::Scheduler(SchedulerEvent::Stats { state }) = envelope.event {
                    stats.push(state);
                }
            }
            Err(TryRecvError::Empty) => return Ok(stats),
            Err(TryRecvError::Closed) => {
                return Err(TestFailure::boxed(String::from("event stream closed")));
            }
            Err(TryRecvError::Lagged(count)) => {
                return Err(TestFailure::boxed(format!(
                    "event stream lagged by {count}"
                )));
            }
        }
    }
}

async fn wait_for_recorded_requests(
    bmc: &RecordingBmc,
    expected: usize,
) -> Result<(), Box<dyn StdError>> {
    for _ in 0..32 {
        if bmc.request_count()? >= expected {
            return Ok(());
        }
        yield_now().await;
    }
    Err(TestFailure::boxed(format!(
        "expected at least {expected} requests, saw {}",
        bmc.request_count()?
    )))
}

async fn wait_for_blocking_request_count(
    bmc: &BlockingBmc,
    expected: usize,
) -> Result<(), Box<dyn StdError>> {
    for _ in 0..32 {
        if bmc.request_count()? >= expected {
            return Ok(());
        }
        yield_now().await;
    }
    Err(TestFailure::boxed(format!(
        "expected at least {expected} requests, saw {}",
        bmc.request_count()?
    )))
}

mod basics;
mod discovery;
mod query;
mod relations;
mod scheduler;
mod store_events;
mod subscription;
