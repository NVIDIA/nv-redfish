// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Sensor abstraction for Redfish entities.
//!
//! This module provides a unified interface for accessing sensor data from
//! Redfish entities that support modern sensor links. The [`HasSensors`] trait
//! is implemented by entities that have associated sensors, and provides access
//! to a [`Sensor`] handle for sensor data retrieval.
//!
//! # Modern vs Legacy Approach
//!
//! This module supports the modern Redfish approach where entities have direct
//! links to their sensors. For legacy BMCs that only expose sensor data through
//! `Chassis/Power` and `Chassis/Thermal`, use those explicit endpoints instead.

use std::sync::Arc;

use nv_redfish_core::{Bmc, NavProperty, ODataId};

use crate::schema::redfish::environment_metrics::EnvironmentMetrics;
use crate::schema::redfish::sensor::{Sensor as SchemaSensor};
use crate::Error;

/// Extracts sensor URIs from metric fields and creates sensor navigation properties.
///
/// Handles both single `Option<SensorExcerpt*>` and `Option<Vec<SensorExcerpt*>>` fields.
/// All `single:` fields must come before `vec:` fields.
///
/// # Example
/// ```ignore
/// extract_sensor_uris!(metrics,
///     single: temperature,
///     single: voltage,
///     vec: fan_speeds
/// )
/// ```
#[macro_export(local_inner_macros)]
macro_rules! extract_sensor_uris {
    ($metrics:expr, $(single: $single_field:ident),* $(, vec: $vec_field:ident)* $(,)?) => {{
        let mut uris = Vec::new();

        $(
            if let Some(Some(uri)) = $metrics.$single_field.as_ref()
                .and_then(|f| f.data_source_uri.as_ref()) {
                uris.push(uri.clone());
            }
        )*

        $(
            if let Some(items) = &$metrics.$vec_field {
                for item in items {
                    if let Some(Some(uri)) = item.data_source_uri.as_ref() {
                        uris.push(uri.clone());
                    }
                }
            }
        )*

        $crate::sensors::collect_sensors(uris)
    }};
}


/// Handle for accessing sensor.
///
/// This struct provides methods to fetch sensor data from the BMC.
/// call to [`fetch`](Self::fetch).
pub struct Sensor<B: Bmc> {
    sensor_ref: NavProperty<SchemaSensor>,
    bmc: Arc<B>,
}

impl<B: Bmc> Sensor<B> {
    /// Create a new sensor handle.
    ///
    /// # Arguments
    ///
    /// * `sensor_ref` - Navigation properties pointing to sensor
    /// * `bmc` - BMC client for fetching sensor data
    #[must_use]
    pub fn new(sensor_ref: NavProperty<SchemaSensor>, bmc: Arc<B>) -> Self {
        Self { sensor_ref, bmc }
    }

    /// Refresh sensor data from the BMC.
    ///
    /// Fetches current sensor readings from the BMC.
    /// This method performs network I/O and may take time to complete.
    ///
    /// # Errors
    ///
    /// Returns an error if sensor fetch fails.
    pub async fn fetch(&self) -> Result<Arc<SchemaSensor>, Error<B>> {
        let sensor = self
            .sensor_ref
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        Ok(sensor)
    }

    /// `OData` identifier of the `NavProperty<Sensor>` in Redfish.
    ///
    /// Typically `/redfish/v1/{Chassis}/Sensors/{ID}`.
    #[must_use]
    pub fn odata_id(&self) -> &ODataId {
        self.sensor_ref.id()
    }
}

/// Collect sensor refs from URIs
pub(crate) fn collect_sensors(
    uris: impl IntoIterator<Item = String>,
) -> Vec<NavProperty<SchemaSensor>> {
    use nv_redfish_core::ODataId;
    uris.into_iter()
        .map(|uri| NavProperty::<SchemaSensor>::new_reference(ODataId::from(uri)))
        .collect()
}

/// Helper function to extract enviroment metrics
pub(crate) async fn extract_environment_sensors<B: Bmc>(
    metrics_ref: &NavProperty<EnvironmentMetrics>,
    bmc: &B,
) -> Vec<NavProperty<SchemaSensor>> {
    metrics_ref
        .get(bmc)
        .await
        .ok()
        .map(|m| {
            extract_sensor_uris!(m,
                single: temperature_celsius,
                single: humidity_percent,
                single: power_watts,
                single: energyk_wh,
                single: power_load_percent,
                single: dew_point_celsius,
                single: absolute_humidity,
                single: energy_joules,
                single: ambient_temperature_celsius,
                single: voltage,
                single: current_amps,
                vec: fan_speeds_percent
            )
        })
        .unwrap_or_default()
}
