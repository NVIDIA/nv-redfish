// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::collections::VecDeque;

use nv_redfish::schema::chassis::Chassis;
use nv_redfish::schema::chassis_collection::ChassisCollection;
use nv_redfish::schema::environment_metrics::EnvironmentMetrics;
use nv_redfish::schema::sensor;
use nv_redfish::schema::sensor_collection::SensorCollection;
use nv_redfish::schema::service_root::ServiceRoot;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef as _;
use nv_redfish_core::ODataId;

use super::DiscoveryBatch;
use super::TypedDiscoveryContext;
use crate::Error;

pub(super) async fn discover_sensors<B>(
    cx: &TypedDiscoveryContext<'_, B>,
) -> Result<DiscoveryBatch, Error>
where
    B: Bmc,
    B::Error: 'static,
{
    StandardSensorDiscovery::new().discover(cx).await
}

struct StandardSensorDiscovery {
    pending: VecDeque<StandardSensorWork>,
    candidates: BTreeSet<ODataId>,
}

impl StandardSensorDiscovery {
    fn new() -> Self {
        Self {
            pending: VecDeque::from([StandardSensorWork::ServiceRoot]),
            candidates: BTreeSet::new(),
        }
    }

    async fn discover<B>(
        mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<DiscoveryBatch, Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        while let Some(work) = self.pending.pop_front() {
            self.advance(cx, work).await?;
        }
        Ok(DiscoveryBatch::candidates(self.candidates))
    }

    async fn advance<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        work: StandardSensorWork,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        match work {
            StandardSensorWork::ServiceRoot => self.fetch_service_root(cx).await,
            StandardSensorWork::ChassisCollection(id) => {
                self.fetch_chassis_collection(cx, id).await
            }
            StandardSensorWork::Chassis(id) => self.fetch_chassis(cx, id).await,
            StandardSensorWork::SensorCollection(id) => self.fetch_sensor_collection(cx, id).await,
            StandardSensorWork::EnvironmentMetrics(id) => {
                self.fetch_environment_metrics(cx, id).await
            }
        }
    }

    async fn fetch_service_root<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let root = cx.fetch::<ServiceRoot>(ODataId::service_root()).await?;
        if let Some(chassis_collection_ref) = &root.chassis {
            self.pending
                .push_back(StandardSensorWork::ChassisCollection(
                    chassis_collection_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_chassis_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let chassis_collection = cx.fetch::<ChassisCollection>(id).await?;
        self.pending.extend(
            chassis_collection
                .members
                .iter()
                .map(|chassis_ref| StandardSensorWork::Chassis(chassis_ref.odata_id().clone())),
        );
        Ok(())
    }

    async fn fetch_chassis<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let chassis = cx.fetch::<Chassis>(id).await?;
        if let Some(sensors_ref) = &chassis.sensors {
            self.pending.push_back(StandardSensorWork::SensorCollection(
                sensors_ref.odata_id().clone(),
            ));
        }
        if let Some(metrics_ref) = &chassis.environment_metrics {
            self.pending
                .push_back(StandardSensorWork::EnvironmentMetrics(
                    metrics_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_sensor_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let sensor_collection = cx.fetch::<SensorCollection>(id).await?;
        self.candidates.extend(
            sensor_collection
                .members
                .iter()
                .map(|sensor_ref| sensor_ref.odata_id().clone()),
        );
        Ok(())
    }

    async fn fetch_environment_metrics<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let metrics = cx.fetch::<EnvironmentMetrics>(id).await?;
        collect_environment_metric_ids(&metrics, &mut self.candidates);
        Ok(())
    }
}

enum StandardSensorWork {
    ServiceRoot,
    ChassisCollection(ODataId),
    Chassis(ODataId),
    SensorCollection(ODataId),
    EnvironmentMetrics(ODataId),
}

fn collect_environment_metric_ids(
    metrics: &EnvironmentMetrics,
    candidates: &mut BTreeSet<ODataId>,
) {
    insert_metric_id(metrics.temperature_celsius.as_ref(), candidates);
    insert_metric_id(metrics.humidity_percent.as_ref(), candidates);
    insert_metric_id(metrics.power_watts.as_ref(), candidates);
    insert_metric_id(metrics.energyk_wh.as_ref(), candidates);
    insert_metric_id(metrics.power_load_percent.as_ref(), candidates);
    insert_metric_id(metrics.dew_point_celsius.as_ref(), candidates);
    insert_metric_id(metrics.absolute_humidity.as_ref(), candidates);
    insert_metric_id(metrics.energy_joules.as_ref(), candidates);
    insert_metric_id(metrics.ambient_temperature_celsius.as_ref(), candidates);
    insert_metric_id(metrics.voltage.as_ref(), candidates);
    insert_metric_id(metrics.current_amps.as_ref(), candidates);
    if let Some(fan_speeds) = &metrics.fan_speeds_percent {
        for fan_speed in fan_speeds {
            insert_metric_id(Some(fan_speed), candidates);
        }
    }
}

fn insert_metric_id<T>(metric: Option<&T>, candidates: &mut BTreeSet<ODataId>)
where
    T: MetricDataSource,
{
    if let Some(uri) = metric.and_then(MetricDataSource::data_source_uri) {
        candidates.insert(ODataId::from(uri.clone()));
    }
}

trait MetricDataSource {
    fn data_source_uri(&self) -> Option<&String>;
}

macro_rules! impl_metric_data_source {
    ($($metric:ty),* $(,)?) => {
        $(
            impl MetricDataSource for $metric {
                fn data_source_uri(&self) -> Option<&String> {
                    self.data_source_uri.as_ref().and_then(Option::as_ref)
                }
            }
        )*
    };
}

impl_metric_data_source!(
    sensor::SensorExcerpt,
    sensor::SensorExcerptCurrent,
    sensor::SensorExcerptEnergykWh,
    sensor::SensorExcerptFanArray,
    sensor::SensorExcerptPower,
    sensor::SensorExcerptVoltage,
);
