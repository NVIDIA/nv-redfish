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
//! Redfish entities that support modern sensor links. The `HasSensors` trait
//! is implemented by entities that have associated sensors, and provides access
//! to a `Sensor` handle for sensor data retrieval.
//!
//! # Modern vs Legacy Approach
//!
//! This module supports the modern Redfish approach where entities have direct
//! links to their sensors. For legacy BMCs that only expose sensor data through
//! `Chassis/Power` and `Chassis/Thermal`, use those explicit endpoints instead.

use crate::entity_link::EntityLink;
use crate::schema::sensor::Sensor as SchemaSensor;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataId;

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

        $crate::sensor::collect_sensors(uris)
    }};
}
/// Link for accessing sensor.
pub type SensorLink<B> = EntityLink<B, SchemaSensor>;

/// Collect sensor refs from URIs
pub(crate) fn collect_sensors(
    uris: impl IntoIterator<Item = String>,
) -> Vec<NavProperty<SchemaSensor>> {
    uris.into_iter()
        .map(|uri| NavProperty::<SchemaSensor>::new_reference(ODataId::from(uri)))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::schema::control::ControlExcerptSingle;
    use crate::schema::environment_metrics::EnvironmentMetrics;

    #[test]
    fn power_limit_watts_uses_control_excerpt() -> Result<(), Box<dyn std::error::Error>> {
        let metrics: EnvironmentMetrics = serde_json::from_str(
            r##"{
                "@odata.id": "/redfish/v1/Chassis/1/EnvironmentMetrics",
                "@odata.type": "#EnvironmentMetrics.v1_1_0.EnvironmentMetrics",
                "Id": "EnvironmentMetrics",
                "Name": "Environment Metrics",
                "PowerLimitWatts": {
                    "DataSourceUri": "/redfish/v1/Chassis/1/Controls/PowerLimit",
                    "SetPoint": 700
                }
            }"##,
        )?;
        let Some(power_limit): Option<ControlExcerptSingle> = metrics.power_limit_watts else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "missing PowerLimitWatts",
            )
            .into());
        };

        assert_eq!(
            power_limit
                .data_source_uri
                .as_ref()
                .and_then(Option::as_deref),
            Some("/redfish/v1/Chassis/1/Controls/PowerLimit")
        );
        assert_eq!(power_limit.set_point, Some(Some(700.0)));

        Ok(())
    }
}
