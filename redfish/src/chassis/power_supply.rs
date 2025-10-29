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

use std::sync::Arc;

use nv_redfish_core::Bmc;

use crate::extract_sensor_uris;
use crate::schema::redfish::power_supply::PowerSupply as PowerSupplySchema;
use crate::schema::redfish::power_supply_metrics::PowerSupplyMetrics;
use crate::sensors::Sensor;
use crate::Error;

/// Represents a power supply in a chassis.
///
/// Provides access to power supply information and associated metrics/sensors.
pub struct PowerSupply<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<PowerSupplySchema>,
}

impl<B> PowerSupply<B>
where
    B: Bmc + Sync + Send,
{
    /// Create a new power supply handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<PowerSupplySchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this power supply.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<PowerSupplySchema> {
        self.data.clone()
    }

    /// Get power supply metrics.
    ///
    /// Returns the power supply's performance and state metrics if available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The power supply does not have metrics
    /// - Fetching metrics data fails
    pub async fn metrics(&self) -> Result<Arc<PowerSupplyMetrics>, Error<B>> {
        let metrics_ref = self
            .data
            .metrics
            .as_ref()
            .ok_or(Error::MetricsNotAvailable)?;

        metrics_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }

    /// Get the metrics sensors for this power supply.
    ///
    /// Returns a vector of `Sensor<B>` obtained from metrics metrics, if available.
    pub async fn metrics_sensors(&self) -> Vec<Sensor<B>> {
        let sensor_refs = if let Some(metrics_ref) = &self.data.metrics {
            metrics_ref
                .get(self.bmc.as_ref())
                .await
                .ok()
                .map(|m| {
                    extract_sensor_uris!(m,
                        single: input_voltage,
                        single: input_current_amps,
                        single: input_power_watts,
                        single: energyk_wh,
                        single: frequency_hz,
                        single: output_power_watts,
                        single: temperature_celsius,
                        single: fan_speed_percent,
                        vec: rail_voltage,
                        vec: rail_current_amps,
                        vec: rail_power_watts,
                        vec: fan_speeds_percent
                    )
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        sensor_refs
            .into_iter()
            .map(|r| Sensor::new(r, self.bmc.clone()))
            .collect()
    }
}
