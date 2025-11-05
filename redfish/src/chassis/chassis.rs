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

use crate::schema::redfish::chassis::Chassis as ChassisSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::bmc::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "power")]
use crate::chassis::Power;
#[cfg(feature = "power-supplies")]
use crate::chassis::PowerSupply;
#[cfg(feature = "thermal")]
use crate::chassis::Thermal;
#[cfg(feature = "log-services")]
use crate::log_services::LogService;
#[cfg(feature = "sensors")]
use crate::schema::redfish::sensor::Sensor as SchemaSensor;
#[cfg(feature = "sensors")]
use crate::sensors::extract_environment_sensors;
#[cfg(feature = "sensors")]
use crate::sensors::Sensor;

/// Represents a chassis in the BMC.
///
/// Provides access to chassis information and sub-resources such as power supplies.
pub struct Chassis<B: Bmc> {
    #[allow(dead_code)] // used if any feature enabled.
    bmc: NvBmc<B>,
    data: Arc<ChassisSchema>,
}

impl<B: Bmc> Chassis<B> {
    /// Create a new chassis handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ChassisSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
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
    /// Attempts to fetch power supplies from `PowerSubsystem` (modern API)
    /// with fallback to Power resource (deprecated API).
    ///
    /// # Errors
    ///
    /// Returns an error if fetching power supply data fails.
    #[cfg(feature = "power-supplies")]
    pub async fn power_supplies(&self) -> Result<Vec<PowerSupply<B>>, Error<B>> {
        if let Some(ps) = &self.data.power_subsystem {
            let ps = ps.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            if let Some(supplies) = &ps.power_supplies {
                let supplies = &self.bmc.expand_property(supplies).await?.members;
                let mut power_supplies = Vec::with_capacity(supplies.len());
                for power_supply in supplies {
                    power_supplies.push(PowerSupply::new(&self.bmc, power_supply).await?);
                }
                return Ok(power_supplies);
            }
        }

        Ok(Vec::new())
    }

    /// Get legacy Power resource (for older BMCs).
    ///
    /// Returns the deprecated `Chassis/Power` resource if available.
    /// For modern BMCs, prefer using direct sensor links via `HasSensors`
    /// or the modern `PowerSubsystem` API.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching power data fails.
    #[cfg(feature = "power")]
    pub async fn power(&self) -> Result<Option<Power<B>>, Error<B>> {
        if let Some(power_ref) = &self.data.power {
            Ok(Some(Power::new(&self.bmc, power_ref).await?))
        } else {
            Ok(None)
        }
    }

    /// Get legacy Thermal resource (for older BMCs).
    ///
    /// Returns the deprecated `Chassis/Thermal` resource if available.
    /// For modern BMCs, prefer using direct sensor links via `HasSensors`
    /// or the modern `ThermalSubsystem` API.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching thermal data fails.
    #[cfg(feature = "thermal")]
    pub async fn thermal(&self) -> Result<Option<Thermal<B>>, Error<B>> {
        if let Some(thermal_ref) = &self.data.thermal {
            Thermal::new(&self.bmc, thermal_ref).await.map(Some)
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
    #[cfg(feature = "log-services")]
    pub async fn log_services(&self) -> Result<Vec<LogService<B>>, Error<B>> {
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
        for m in &log_services_collection.members {
            log_services.push(LogService::new(&self.bmc, m).await?);
        }

        Ok(log_services)
    }

    /// Get the environment sensors for this chassis.
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

    /// Get the sensors collection for this chassis.
    ///
    /// Returns all available sensors associated with the chassis.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The chassis does not have sensors
    /// - Fetching sensors data fails
    #[cfg(feature = "sensors")]
    pub async fn sensors(&self) -> Result<Vec<Sensor<B>>, Error<B>> {
        if let Some(sensors_collection) = &self.data.sensors {
            let sc = sensors_collection
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            let mut sensor_data = Vec::with_capacity(sc.members.len());
            for sensor in &sc.members {
                sensor_data.push(Sensor::new(
                    self.bmc.clone(),
                    NavProperty::<SchemaSensor>::new_reference(sensor.id().clone()),
                ));
            }
            Ok(sensor_data)
        } else {
            Err(Error::SensorsNotAvailable)
        }
    }
}
