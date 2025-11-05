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

use crate::schema::redfish::drive::Drive as DriveSchema;
use crate::schema::redfish::drive_metrics::DriveMetrics;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "sensors")]
use crate::sensors::extract_environment_sensors;
#[cfg(feature = "sensors")]
use crate::sensors::Sensor;

/// Represents a drive (disk) in a storage controller.
///
/// Provides access to drive information and associated metrics/sensors.
pub struct Drive<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<DriveSchema>,
}

impl<B: Bmc> Drive<B> {
    /// Create a new drive handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<DriveSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    /// Get the raw schema data for this drive.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<DriveSchema> {
        self.data.clone()
    }

    /// Get drive metrics.
    ///
    /// Returns the drive's performance and state metrics if available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The drive does not have metrics
    /// - Fetching metrics data fails
    pub async fn metrics(&self) -> Result<Arc<DriveMetrics>, Error<B>> {
        let metrics_ref = self
            .data
            .metrics
            .as_ref()
            .ok_or(Error::MetricsNotAvailable)?;

        metrics_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }

    /// Get the environment sensors for this drive.
    ///
    /// Returns a vector of `Sensor<B>` obtained from environment metrics, if available.
    ///
    /// # Errors
    ///
    /// Returns an error if get of environment metrics failed.
    #[cfg(feature = "sensors")]
    pub async fn environment_sensors(&self) -> Result<Vec<Sensor<B>>, Error<B>> {
        let sensor_refs = if let Some(env_ref) = &self.data.environment_metrics {
            extract_environment_sensors(env_ref, self.bmc.as_ref()).await?
        } else {
            Vec::new()
        };

        Ok(sensor_refs
            .into_iter()
            .map(|r| Sensor::new(self.bmc.clone(), r))
            .collect())
    }
}
