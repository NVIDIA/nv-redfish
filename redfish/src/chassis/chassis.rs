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

use nv_redfish_core::{http::ExpandQuery, Bmc, Expandable};
use nv_redfish_core::{NavProperty, ODataId};

use crate::chassis::PowerSupply;
use crate::schema::redfish::chassis::Chassis as ChassisSchema;
use crate::schema::redfish::sensor::Sensor as SchemaSensor;
use crate::sensors::Sensor;
use crate::Error;

/// Represents a chassis in the BMC.
///
/// Provides access to chassis information and sub-resources such as power supplies.
pub struct Chassis<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<ChassisSchema>,
}

impl<B> Chassis<B>
where
    B: Bmc + Sync + Send,
{
    /// Create a new chassis handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<ChassisSchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this chassis.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<ChassisSchema> {
        self.data.clone()
    }

    /// Get power supplies from this chassis.
    ///
    /// Attempts to fetch power supplies from PowerSubsystem (modern API)
    /// with fallback to Power resource (deprecated API).
    ///
    /// # Errors
    ///
    /// Returns an error if fetching power supply data fails.
    pub async fn get_power_supplies(&self) -> Result<Vec<PowerSupply<B>>, Error<B>> {
        if let Some(ps) = &self.data.power_subsystem {
            let ps = ps.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            if let Some(supplies) = &ps.power_supplies {
                let supplies = &supplies
                    .expand(self.bmc.as_ref(), ExpandQuery::all())
                    .await
                    .map_err(Error::Bmc)?
                    .get(self.bmc.as_ref())
                    .await
                    .map_err(Error::Bmc)?
                    .members;
                let mut power_supplies = Vec::with_capacity(supplies.len());
                for power_supply in supplies {
                    let power_supply = power_supply
                        .get(self.bmc.as_ref())
                        .await
                        .map_err(Error::Bmc)?;
                    power_supplies.push(PowerSupply::new(self.bmc.clone(), power_supply));
                }
                return Ok(power_supplies);
            }
        }

        Ok(Vec::new())
    }

    /// Get legacy Power resource (for older BMCs).
    ///
    /// Returns the deprecated `Chassis/Power` resource if available.
    /// For modern BMCs, prefer using direct sensor links via [`HasSensors`]
    /// or the modern PowerSubsystem API.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching power data fails.
    pub async fn power(&self) -> Result<Option<crate::chassis::Power<B>>, Error<B>> {
        if let Some(power_ref) = &self.data.power {
            let power = power_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            Ok(Some(crate::chassis::Power::new(self.bmc.clone(), power)))
        } else {
            Ok(None)
        }
    }

    /// Get legacy Thermal resource (for older BMCs).
    ///
    /// Returns the deprecated `Chassis/Thermal` resource if available.
    /// For modern BMCs, prefer using direct sensor links via [`HasSensors`]
    /// or the modern ThermalSubsystem API.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching thermal data fails.
    pub async fn thermal(&self) -> Result<Option<crate::chassis::Thermal<B>>, Error<B>> {
        if let Some(thermal_ref) = &self.data.thermal {
            let thermal = thermal_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            Ok(Some(crate::chassis::Thermal::new(
                self.bmc.clone(),
                thermal,
            )))
        } else {
            Ok(None)
        }
    }

    /// Get log services for this chassis.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The chassis does not have log services
    /// - Fetching log service data fails
    #[cfg(feature = "__log_service")]
    pub async fn list_log_services(
        &self,
    ) -> Result<Vec<crate::log_service::LogService<B>>, Error<B>> {
        let log_services_ref = self
            .data
            .log_services
            .as_ref()
            .ok_or(Error::LogServiceNotAvailable)?;

        let log_services_collection = log_services_ref
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut log_services = Vec::new();
        for log_service_ref in &log_services_collection.members {
            let log_service = log_service_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            log_services.push(crate::log_service::LogService::new(
                self.bmc.clone(),
                log_service,
            ));
        }

        Ok(log_services)
    }

    /// Get the environment sensors for this chassis.
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

    /// Get the sensors collection for this chassis.
    ///
    /// Returns all available sensors associated with the chassis.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The chassis does not have sensors
    /// - Fetching sensors data fails
    pub async fn sensors(&self) -> Result<Vec<Sensor<B>>, Error<B>> {
        if let Some(sensors_collection) = &self.data.sensors {
            let sc = sensors_collection
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            let mut sensor_data = Vec::with_capacity(sc.members.len());
            for sensor in &sc.members {
                sensor_data.push(Sensor::new(
                    NavProperty::<SchemaSensor>::new_reference(ODataId::from(sensor.id().clone())),
                    self.bmc.clone(),
                ));
            }
            Ok(sensor_data)
        } else {
            Err(Error::SensorsNotAvailable)
        }
    }
}
