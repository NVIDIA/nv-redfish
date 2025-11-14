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

use crate::schema::redfish::processor::Processor as ProcessorSchema;
use crate::schema::redfish::processor_metrics::ProcessorMetrics;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "sensors")]
use crate::extract_sensor_uris;
#[cfg(feature = "sensors")]
use crate::sensors::extract_environment_sensors;
#[cfg(feature = "sensors")]
use crate::sensors::SensorRef;

/// Represents a processor in a computer system.
///
/// Provides access to processor information and associated metrics/sensors.
pub struct Processor<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<ProcessorSchema>,
}

impl<B: Bmc> Processor<B> {
    /// Create a new processor handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ProcessorSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    /// Get the raw schema data for this processor.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<ProcessorSchema> {
        self.data.clone()
    }

    /// Get processor metrics.
    ///
    /// Returns the processor's performance and state metrics if available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The processor does not have metrics
    /// - Fetching metrics data fails
    pub async fn metrics(&self) -> Result<Arc<ProcessorMetrics>, Error<B>> {
        let metrics_ref = self
            .data
            .metrics
            .as_ref()
            .ok_or(Error::MetricsNotAvailable)?;

        metrics_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }

    /// Get the environment sensors for this processor.
    ///
    /// Returns a vector of `Sensor<B>` obtained from environment metrics, if available.
    ///
    /// # Errors
    ///
    /// Returns an error if get of environment metrics failed.
    #[cfg(feature = "sensors")]
    pub async fn environment_sensors(&self) -> Result<Vec<SensorRef<B>>, Error<B>> {
        let sensor_refs = if let Some(env_ref) = &self.data.environment_metrics {
            extract_environment_sensors(env_ref, self.bmc.as_ref()).await?
        } else {
            Vec::new()
        };

        Ok(sensor_refs
            .into_iter()
            .map(|r| SensorRef::new(self.bmc.clone(), r))
            .collect())
    }

    /// Get the metrics sensors for this processor.
    ///
    /// Returns a vector of `Sensor<B>` obtained from metrics metrics, if available.
    ///
    /// # Errors
    ///
    /// Returns an error if get of metrics failed.
    #[cfg(feature = "sensors")]
    pub async fn metrics_sensors(&self) -> Result<Vec<SensorRef<B>>, Error<B>> {
        let sensor_refs = if let Some(metrics_ref) = &self.data.metrics {
            metrics_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)
                .map(|m| {
                    extract_sensor_uris!(m,
                        single: core_voltage,
                    )
                })?
        } else {
            Vec::new()
        };

        Ok(sensor_refs
            .into_iter()
            .map(|r| SensorRef::new(self.bmc.clone(), r))
            .collect())
    }
}

impl<B: Bmc> Resource for Processor<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
