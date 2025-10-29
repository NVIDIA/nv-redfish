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

use crate::schema::redfish::memory::Memory as MemorySchema;
use crate::schema::redfish::memory_metrics::MemoryMetrics;
use crate::sensors::Sensor;
use crate::Error;

/// Represents a memory module (DIMM) in a computer system.
///
/// Provides access to memory module information and associated metrics/sensors.
pub struct Memory<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<MemorySchema>,
}

impl<B> Memory<B>
where
    B: Bmc + Sync + Send,
{
    /// Create a new memory handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<MemorySchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this memory module.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<MemorySchema> {
        self.data.clone()
    }

    /// Get memory metrics.
    ///
    /// Returns the memory module's performance and state metrics if available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The memory module does not have metrics
    /// - Fetching metrics data fails
    pub async fn metrics(&self) -> Result<Arc<MemoryMetrics>, Error<B>> {
        let metrics_ref = self
            .data
            .metrics
            .as_ref()
            .ok_or(Error::MetricsNotAvailable)?;

        metrics_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }

    /// Get the environment sensors for this memory.
    ///
    /// Returns a vector of `Sensor<B>` obtained from environment metrics, if available.
    pub async fn environment_sensors(&self) -> Vec<Sensor<B>> {
        let sensor_refs = if let Some(env_ref) = &self.data.environment_metrics {
            crate::sensors::extract_environment_sensors(env_ref, self.bmc.as_ref()).await
        } else {
            Vec::new()
        };

        sensor_refs
            .into_iter()
            .map(|r| Sensor::new(r, self.bmc.clone()))
            .collect()
    }
}
