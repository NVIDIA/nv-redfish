// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;
use std::collections::BTreeMap;

use nv_redfish::schema::log_service::LogService;
use nv_redfish::schema::sensor::ReadingType;
use nv_redfish::schema::sensor::Sensor;
use nv_redfish::schema::sensor::Threshold;
use nv_redfish::schema::sensor::Thresholds;
use nv_redfish::schema::software_inventory::SoftwareInventory;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use nv_redfish_core::ToSnakeCase;
use nv_redfish_scraper::Relation;
use nv_redfish_scraper::ResourceSnapshot;
use nv_redfish_scraper::TypedResourceEvent;

/// Health collector event shape used by this integration example.
#[derive(Clone, Debug, PartialEq)]
pub enum CollectorEvent {
    /// Sensor metric update.
    Metric(HealthMetric),
    /// Firmware inventory update.
    Firmware(FirmwareInfo),
    /// Log service inventory update.
    LogService(LogServiceInfo),
}

/// Health-side metric event produced from scraper snapshots.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthMetric {
    /// Health-owned metric name.
    pub name: String,
    /// Sensor resource id.
    pub sensor_id: ODataId,
    /// Sensor reading value.
    pub reading: Option<f64>,
    /// Sanitized reading unit.
    pub unit: Option<String>,
    /// Health-owned labels.
    pub labels: BTreeMap<String, String>,
    /// Threshold values preserved from Redfish sensor data.
    pub thresholds: SensorThresholds,
}

/// Health-side threshold values carried on metric events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SensorThresholds {
    /// Upper caution threshold.
    pub upper_caution: Option<f64>,
    /// Upper critical threshold.
    pub upper_critical: Option<f64>,
    /// Upper fatal threshold.
    pub upper_fatal: Option<f64>,
    /// Lower caution threshold.
    pub lower_caution: Option<f64>,
    /// Lower critical threshold.
    pub lower_critical: Option<f64>,
    /// Lower fatal threshold.
    pub lower_fatal: Option<f64>,
}

/// Health-side firmware information produced from scraper snapshots.
#[derive(Clone, Debug, PartialEq)]
pub struct FirmwareInfo {
    /// Firmware resource id.
    pub id: ODataId,
    /// Firmware version reported by Redfish.
    pub version: Option<String>,
}

/// Health-side log service information produced from scraper snapshots.
#[derive(Clone, Debug, PartialEq)]
pub struct LogServiceInfo {
    /// Log service resource id.
    pub id: ODataId,
    /// Optional log entry collection id.
    pub entries: Option<ODataId>,
}

/// Minimal health sink trait kept outside scraper core.
pub trait HealthSink {
    /// Emits one health collector event.
    fn emit(&mut self, event: CollectorEvent);
}

/// Health-side projection from typed scraper sensor events into metric events.
#[derive(Clone, Debug, Default)]
pub struct SensorHealthProjection {
    relation_labels: BTreeMap<ODataId, BTreeMap<String, String>>,
}

impl SensorHealthProjection {
    /// Records relation metadata as health labels.
    ///
    /// Relation interpretation is intentionally health-side policy. The scraper
    /// only supplies typed relation data.
    pub fn record_relation(&mut self, relation: &Relation) {
        if relation.from.type_id != TypeId::of::<Sensor>() {
            return;
        }
        self.relation_labels
            .entry(relation.from.id.clone())
            .or_default()
            .insert(String::from("related_resource"), relation.to.id.to_string());
    }

    /// Converts a typed scraper sensor event into a health collector event.
    #[must_use]
    pub fn project(&self, event: &TypedResourceEvent<Sensor>) -> Option<CollectorEvent> {
        match event {
            TypedResourceEvent::Added(snapshot) => self.metric_event(snapshot),
            TypedResourceEvent::Updated { new, .. } => self.metric_event(new),
            TypedResourceEvent::Removed(_)
            | TypedResourceEvent::FreshnessMissed { .. }
            | TypedResourceEvent::Error { .. } => None,
        }
    }

    /// Converts and emits a health metric event through a health-owned sink.
    pub fn emit_projected<S>(&self, event: &TypedResourceEvent<Sensor>, sink: &mut S)
    where
        S: HealthSink,
    {
        if let Some(event) = self.project(event) {
            sink.emit(event);
        }
    }

    fn metric_event(&self, snapshot: &ResourceSnapshot<Sensor>) -> Option<CollectorEvent> {
        let reading = flatten_optional(snapshot.value.reading);
        let mut labels = self.base_labels(snapshot);
        if let Some(reading_type) = flatten_optional(snapshot.value.reading_type) {
            labels.insert(
                String::from("reading_type"),
                reading_type.to_snake_case().to_owned(),
            );
        }
        Some(CollectorEvent::Metric(HealthMetric {
            name: metric_name(flatten_optional(snapshot.value.reading_type)),
            sensor_id: snapshot.id.clone(),
            reading,
            unit: flatten_optional_ref(&snapshot.value.reading_units)
                .map(|unit| sanitize_unit(unit)),
            labels,
            thresholds: thresholds(snapshot.value.thresholds.as_ref()),
        }))
    }

    fn base_labels(&self, snapshot: &ResourceSnapshot<Sensor>) -> BTreeMap<String, String> {
        let mut labels = self
            .relation_labels
            .get(&snapshot.id)
            .cloned()
            .unwrap_or_default();
        labels.insert(String::from("sensor_id"), snapshot.id.to_string());
        labels
    }
}

/// Health-side projection from firmware scraper events.
#[derive(Clone, Copy, Debug, Default)]
pub struct FirmwareHealthProjection;

impl FirmwareHealthProjection {
    /// Converts a typed scraper firmware event into a health collector event.
    #[must_use]
    pub fn project(self, event: &TypedResourceEvent<SoftwareInventory>) -> Option<CollectorEvent> {
        match event {
            TypedResourceEvent::Added(snapshot)
            | TypedResourceEvent::Updated { new: snapshot, .. } => {
                Some(CollectorEvent::Firmware(FirmwareInfo {
                    id: snapshot.id.clone(),
                    version: snapshot.value.version.clone().flatten(),
                }))
            }
            TypedResourceEvent::Removed(_)
            | TypedResourceEvent::FreshnessMissed { .. }
            | TypedResourceEvent::Error { .. } => None,
        }
    }
}

/// Health-side projection from log service scraper events.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogServiceHealthProjection;

impl LogServiceHealthProjection {
    /// Converts a typed scraper log service event into a health collector event.
    #[must_use]
    pub fn project(self, event: &TypedResourceEvent<LogService>) -> Option<CollectorEvent> {
        match event {
            TypedResourceEvent::Added(snapshot)
            | TypedResourceEvent::Updated { new: snapshot, .. } => {
                Some(CollectorEvent::LogService(LogServiceInfo {
                    id: snapshot.id.clone(),
                    entries: snapshot
                        .value
                        .entries
                        .as_ref()
                        .map(|entries| entries.odata_id().clone()),
                }))
            }
            TypedResourceEvent::Removed(_)
            | TypedResourceEvent::FreshnessMissed { .. }
            | TypedResourceEvent::Error { .. } => None,
        }
    }
}

fn metric_name(reading_type: Option<ReadingType>) -> String {
    let suffix = reading_type.map_or("unknown", |value| value.to_snake_case());
    format!("redfish.sensor.{suffix}")
}

fn sanitize_unit(unit: &str) -> String {
    unit.replace(['{', '}', '[', ']'], "")
        .replace('/', "_per_")
        .replace('%', "percent")
}

fn thresholds(thresholds: Option<&Thresholds>) -> SensorThresholds {
    let Some(thresholds) = thresholds else {
        return SensorThresholds::default();
    };
    SensorThresholds {
        upper_caution: threshold_reading(thresholds.upper_caution.as_ref()),
        upper_critical: threshold_reading(thresholds.upper_critical.as_ref()),
        upper_fatal: threshold_reading(thresholds.upper_fatal.as_ref()),
        lower_caution: threshold_reading(thresholds.lower_caution.as_ref()),
        lower_critical: threshold_reading(thresholds.lower_critical.as_ref()),
        lower_fatal: threshold_reading(thresholds.lower_fatal.as_ref()),
    }
}

fn threshold_reading(threshold: Option<&Threshold>) -> Option<f64> {
    threshold.and_then(|threshold| flatten_optional(threshold.reading))
}

fn flatten_optional<T>(value: Option<Option<T>>) -> Option<T> {
    value.flatten()
}

fn flatten_optional_ref<T>(value: &Option<Option<T>>) -> Option<&T> {
    value.as_ref().and_then(Option::as_ref)
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::error::Error as StdError;
    use std::fmt::Display;
    use std::fmt::Formatter;
    use std::fmt::Result as FmtResult;
    use std::future::Future;
    use std::pin::Pin;

    use nv_redfish_bmc_mock::Bmc as MockBmc;
    use nv_redfish_bmc_mock::Expect;
    use nv_redfish_core::ODataId;
    use nv_redfish_scraper::BmcCapacity;
    use nv_redfish_scraper::Discoverer;
    use nv_redfish_scraper::Discovery;
    use nv_redfish_scraper::DiscoveryBatch;
    use nv_redfish_scraper::DiscoveryContext;
    use nv_redfish_scraper::DiscoveryHint;
    use nv_redfish_scraper::RelationKind;
    use nv_redfish_scraper::ResourceRef;
    use nv_redfish_scraper::Scraper;
    use serde_json::json;

    use super::*;

    #[derive(Debug)]
    struct RelatedDrive;

    #[derive(Clone, Debug, Default)]
    struct MockError;

    impl Display for MockError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            formatter.write_str("mock error")
        }
    }

    impl StdError for MockError {}

    #[derive(Clone, Debug)]
    struct SensorDiscovery {
        ids: Vec<ODataId>,
    }

    impl Discoverer<Sensor> for SensorDiscovery {
        fn discover<'a>(
            &'a self,
            _cx: &'a mut DiscoveryContext<'a>,
            _hint: DiscoveryHint,
        ) -> Pin<
            Box<dyn Future<Output = Result<DiscoveryBatch, nv_redfish_scraper::Error>> + Send + 'a>,
        > {
            let ids = self.ids.clone();
            Box::pin(async move { Ok(DiscoveryBatch::candidates(ids)) })
        }
    }

    #[derive(Default)]
    struct RecordingHealthSink {
        events: Vec<CollectorEvent>,
    }

    impl HealthSink for RecordingHealthSink {
        fn emit(&mut self, event: CollectorEvent) {
            self.events.push(event);
        }
    }

    #[tokio::test]
    async fn sensor_projection_converts_added_snapshot_to_metric_event(
    ) -> Result<(), Box<dyn StdError>> {
        let sensor_id = id("/redfish/v1/Chassis/1/Sensors/InletTemp");
        let scraper = scraper_with_sensor(&sensor_id, sensor_json(&sensor_id, 41.5, None)).await?;
        let mut sensors = scraper.query::<Sensor>().subscribe().await?;
        let projection = SensorHealthProjection::default();

        let event = sensors.recv().await?;
        let metric = projected_metric(&projection, &event)?;

        assert_eq!(metric.name, "redfish.sensor.temperature");
        assert_eq!(metric.sensor_id, sensor_id);
        assert_eq!(metric.reading, Some(41.5));
        assert_eq!(metric.unit, Some(String::from("Cel")));
        assert_eq!(
            metric.labels.get("reading_type"),
            Some(&String::from("temperature"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn sensor_projection_converts_updated_snapshot_to_metric_event(
    ) -> Result<(), Box<dyn StdError>> {
        let sensor_id = id("/redfish/v1/Chassis/1/Sensors/InletTemp");
        let bmc = MockBmc::<MockError>::default();
        bmc.expect(Expect::get(&sensor_id, sensor_json(&sensor_id, 40.0, None)));
        bmc.expect(Expect::get(
            &sensor_id,
            sensor_json(&sensor_id, 44.25, None),
        ));
        let scraper = build_scraper(bmc, sensor_id.clone()).await?;
        let mut sensors = scraper.query::<Sensor>().subscribe().await?;
        let _initial = sensors.recv().await?;

        let _updated = scraper
            .resources::<Sensor>()
            .refresh(sensor_id.clone())
            .await?;
        let event = sensors.recv().await?;
        let metric = projected_metric(&SensorHealthProjection::default(), &event)?;

        assert_eq!(metric.reading, Some(44.25));
        Ok(())
    }

    #[tokio::test]
    async fn sensor_projection_preserves_threshold_fields() -> Result<(), Box<dyn StdError>> {
        let sensor_id = id("/redfish/v1/Chassis/1/Sensors/InletTemp");
        let scraper = scraper_with_sensor(
            &sensor_id,
            sensor_json(&sensor_id, 41.5, Some((70.0, 80.0))),
        )
        .await?;
        let mut sensors = scraper.query::<Sensor>().subscribe().await?;
        let projection = SensorHealthProjection::default();

        let event = sensors.recv().await?;
        let metric = projected_metric(&projection, &event)?;

        assert_eq!(metric.thresholds.upper_caution, Some(70.0));
        assert_eq!(metric.thresholds.upper_critical, Some(80.0));
        Ok(())
    }

    #[tokio::test]
    async fn sensor_projection_uses_relation_labels() -> Result<(), Box<dyn StdError>> {
        let sensor_id = id("/redfish/v1/Chassis/1/Sensors/DriveTemp");
        let drive_id = id("/redfish/v1/Systems/1/Storage/1/Drives/0");
        let scraper = scraper_with_sensor(&sensor_id, sensor_json(&sensor_id, 37.0, None)).await?;
        let mut sensors = scraper.query::<Sensor>().subscribe().await?;
        let mut projection = SensorHealthProjection::default();
        projection.record_relation(&Relation::new(
            ResourceRef::of::<Sensor>(sensor_id.clone()),
            ResourceRef::new(TypeId::of::<RelatedDrive>(), drive_id.clone()),
            RelationKind::RelatedTo,
        ));

        let event = sensors.recv().await?;
        let metric = projected_metric(&projection, &event)?;

        assert_eq!(
            metric.labels.get("related_resource"),
            Some(&drive_id.to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn firmware_projection_converts_snapshot_to_info() -> Result<(), Box<dyn StdError>> {
        let firmware_id = id("/redfish/v1/UpdateService/FirmwareInventory/BMC");
        let bmc = MockBmc::<MockError>::default();
        bmc.expect(Expect::get(
            &firmware_id,
            firmware_json(&firmware_id, "1.2.3"),
        ));
        let scraper = Scraper::builder(bmc).build().await?;
        let snapshot = scraper
            .resources::<SoftwareInventory>()
            .refresh(firmware_id.clone())
            .await?;

        let event = TypedResourceEvent::Added(snapshot);
        let projected = FirmwareHealthProjection.project(&event);

        assert_eq!(
            projected,
            Some(CollectorEvent::Firmware(FirmwareInfo {
                id: firmware_id,
                version: Some(String::from("1.2.3")),
            }))
        );
        Ok(())
    }

    #[tokio::test]
    async fn log_service_projection_converts_snapshot_to_info() -> Result<(), Box<dyn StdError>> {
        let service_id = id("/redfish/v1/Managers/BMC/LogServices/EventLog");
        let entries_id = id("/redfish/v1/Managers/BMC/LogServices/EventLog/Entries");
        let bmc = MockBmc::<MockError>::default();
        bmc.expect(Expect::get(
            &service_id,
            log_service_json(&service_id, &entries_id),
        ));
        let scraper = Scraper::builder(bmc).build().await?;
        let snapshot = scraper
            .resources::<LogService>()
            .refresh(service_id.clone())
            .await?;

        let event = TypedResourceEvent::Added(snapshot);
        let projected = LogServiceHealthProjection.project(&event);

        assert_eq!(
            projected,
            Some(CollectorEvent::LogService(LogServiceInfo {
                id: service_id,
                entries: Some(entries_id),
            }))
        );
        Ok(())
    }

    #[test]
    fn sensor_projection_does_not_embed_sink_logic_in_scraper() -> Result<(), Box<dyn StdError>> {
        let manifest = std::fs::read_to_string("../../scraper/Cargo.toml")?;
        assert!(!manifest.contains("scraper-health-projection"));
        assert!(!manifest.contains("HealthSink"));
        assert!(!manifest.contains("CollectorEvent"));

        let mut sink = RecordingHealthSink::default();
        sink.emit(CollectorEvent::Metric(HealthMetric {
            name: String::from("redfish.sensor.temperature"),
            sensor_id: id("/redfish/v1/Chassis/1/Sensors/InletTemp"),
            reading: Some(42.0),
            unit: Some(String::from("Cel")),
            labels: BTreeMap::new(),
            thresholds: SensorThresholds::default(),
        }));
        assert_eq!(sink.events.len(), 1);
        Ok(())
    }

    async fn scraper_with_sensor(
        sensor_id: &ODataId,
        sensor: String,
    ) -> Result<Scraper<MockBmc<MockError>>, Box<dyn StdError>> {
        let bmc = MockBmc::<MockError>::default();
        bmc.expect(Expect::get(sensor_id, sensor));
        build_scraper(bmc, sensor_id.clone()).await
    }

    async fn build_scraper(
        bmc: MockBmc<MockError>,
        sensor_id: ODataId,
    ) -> Result<Scraper<MockBmc<MockError>>, Box<dyn StdError>> {
        Ok(Scraper::builder(bmc)
            .capacity(
                BmcCapacity::fixed()
                    .max_in_flight(1)
                    .max_requests_per_second(u32::MAX),
            )
            .discover(Discovery::manual::<Sensor, _>(SensorDiscovery {
                ids: vec![sensor_id],
            }))
            .build()
            .await?)
    }

    fn projected_metric(
        projection: &SensorHealthProjection,
        event: &TypedResourceEvent<Sensor>,
    ) -> Result<HealthMetric, Box<dyn StdError>> {
        let Some(CollectorEvent::Metric(metric)) = projection.project(event) else {
            return Err(String::from("expected metric event").into());
        };
        Ok(metric)
    }

    fn sensor_json(sensor_id: &ODataId, reading: f64, thresholds: Option<(f64, f64)>) -> String {
        let mut sensor = json!({
            "@odata.id": sensor_id,
            "@odata.etag": null,
            "@Redfish.Settings": null,
            "@Redfish.SettingsApplyTime": null,
            "Id": "InletTemp",
            "Name": "Inlet temperature",
            "ReadingType": "Temperature",
            "Reading": reading,
            "ReadingUnits": "Cel"
        });
        if let Some((upper_caution, upper_critical)) = thresholds {
            sensor["Thresholds"] = json!({
                "UpperCaution": {
                    "Reading": upper_caution,
                    "Activation": "Increasing"
                },
                "UpperCritical": {
                    "Reading": upper_critical,
                    "Activation": "Increasing"
                }
            });
        }
        sensor.to_string()
    }

    fn firmware_json(firmware_id: &ODataId, version: &str) -> String {
        json!({
            "@odata.id": firmware_id,
            "@odata.etag": null,
            "@Redfish.Settings": null,
            "@Redfish.SettingsApplyTime": null,
            "Id": "BMC",
            "Name": "BMC firmware",
            "Version": version
        })
        .to_string()
    }

    fn log_service_json(service_id: &ODataId, entries_id: &ODataId) -> String {
        json!({
            "@odata.id": service_id,
            "@odata.etag": null,
            "@Redfish.Settings": null,
            "@Redfish.SettingsApplyTime": null,
            "Id": "EventLog",
            "Name": "Event Log",
            "OverWritePolicy": "WrapsWhenFull",
            "Entries": {
                "@odata.id": entries_id
            }
        })
        .to_string()
    }

    fn id(value: &str) -> ODataId {
        value.to_owned().into()
    }
}
