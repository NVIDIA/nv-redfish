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

use crate::hardware_id::HardwareIdRef;
use crate::hardware_id::Manufacturer as HardwareIdManufacturer;
use crate::hardware_id::Model as HardwareIdModel;
use crate::hardware_id::PartNumber as HardwareIdPartNumber;
use crate::hardware_id::SerialNumber as HardwareIdSerialNumber;
use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::chassis::Chassis as ChassisSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use crate::ServiceRoot;
use nv_redfish_core::bmc::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "assembly")]
use crate::assembly::Assembly;
#[cfg(feature = "assembly")]
use crate::assembly::Config as AssemblyConfig;
#[cfg(feature = "network-adapters")]
use crate::chassis::NetworkAdapter;
#[cfg(feature = "network-adapters")]
use crate::chassis::NetworkAdapterCollection;
#[cfg(feature = "power")]
use crate::chassis::Power;
#[cfg(feature = "power-supplies")]
use crate::chassis::PowerSupply;
#[cfg(feature = "thermal")]
use crate::chassis::Thermal;
#[cfg(feature = "log-services")]
use crate::log_service::LogService;
#[cfg(feature = "oem-nvidia-baseboard")]
use crate::oem::nvidia::baseboard::NvidiaCbcChassis;
#[cfg(feature = "pcie-devices")]
use crate::pcie_device::PcieDeviceCollection;
#[cfg(feature = "sensors")]
use crate::schema::redfish::sensor::Sensor as SchemaSensor;
#[cfg(feature = "sensors")]
use crate::sensor::extract_environment_sensors;
#[cfg(feature = "sensors")]
use crate::sensor::SensorRef;

#[doc(hidden)]
pub enum ChassisTag {}

/// Chassis manufacturer.
pub type Manufacturer<T> = HardwareIdManufacturer<T, ChassisTag>;

/// Chassis model.
pub type Model<T> = HardwareIdModel<T, ChassisTag>;

/// Chassis part number.
pub type PartNumber<T> = HardwareIdPartNumber<T, ChassisTag>;

/// Chassis serial number.
pub type SerialNumber<T> = HardwareIdSerialNumber<T, ChassisTag>;

pub struct Config {
    read_patch_fn: Option<ReadPatchFn>,
    #[cfg(feature = "assembly")]
    assembly: Arc<AssemblyConfig>,
}

impl Config {
    pub fn new<B: Bmc>(root: &ServiceRoot<B>) -> Self {
        let mut patches = Vec::new();
        if root.bug_invalid_contained_by_fields() {
            patches.push(remove_invalid_contained_by_fields);
        }
        let read_patch_fn = if patches.is_empty() {
            None
        } else {
            let read_patch_fn: ReadPatchFn =
                Arc::new(move |v| patches.iter().fold(v, |acc, f| f(acc)));
            Some(read_patch_fn)
        };
        Self {
            read_patch_fn,
            #[cfg(feature = "assembly")]
            assembly: AssemblyConfig::new(root).into(),
        }
    }
}

/// Represents a chassis in the BMC.
///
/// Provides access to chassis information and sub-resources such as power supplies.
pub struct Chassis<B: Bmc> {
    #[allow(dead_code)] // used if any feature enabled.
    bmc: NvBmc<B>,
    data: Arc<ChassisSchema>,
    #[allow(dead_code)] // used when assembly feature enabled.
    config: Arc<Config>,
}

impl<B: Bmc> Chassis<B> {
    /// Create a new chassis handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ChassisSchema>,
        config: Arc<Config>,
    ) -> Result<Self, Error<B>> {
        if let Some(read_patch_fn) = &config.read_patch_fn {
            Payload::get(bmc.as_ref(), nav, read_patch_fn.as_ref()).await
        } else {
            nav.get(bmc.as_ref()).await.map_err(Error::Bmc)
        }
        .map(|data| Self {
            bmc: bmc.clone(),
            data,
            config,
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

    /// Get hardware identifier of the network adpater.
    #[must_use]
    pub fn hardware_id(&self) -> HardwareIdRef<'_, ChassisTag> {
        HardwareIdRef {
            manufacturer: self
                .data
                .manufacturer
                .as_ref()
                .and_then(Option::as_ref)
                .map(Manufacturer::new),
            model: self
                .data
                .model
                .as_ref()
                .and_then(Option::as_ref)
                .map(Model::new),
            part_number: self
                .data
                .part_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(PartNumber::new),
            serial_number: self
                .data
                .serial_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(SerialNumber::new),
        }
    }

    /// Get assembly of this chassis
    ///
    /// # Errors
    ///
    /// Returns an error if fetching assembly data fails.
    #[cfg(feature = "assembly")]
    pub async fn assembly(&self) -> Result<Assembly<B>, Error<B>> {
        let assembly_ref = self
            .data
            .assembly
            .as_ref()
            .ok_or(Error::AssemblyNotAvailable)?;
        Assembly::new(&self.bmc, assembly_ref, &self.config.assembly).await
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

    /// Get network adapter resources
    ///
    /// Returns the `Chassis/NetworkAdapter` resources if available.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching network adapters data fails.
    #[cfg(feature = "network-adapters")]
    pub async fn network_adapters(&self) -> Result<Vec<NetworkAdapter<B>>, Error<B>> {
        let network_adapters_collection_ref = &self
            .data
            .network_adapters
            .as_ref()
            .ok_or(Error::NetworkAdaptersNotAvailable)?;
        NetworkAdapterCollection::new(&self.bmc, network_adapters_collection_ref)
            .await?
            .members()
            .await
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
    pub async fn sensors(&self) -> Result<Vec<SensorRef<B>>, Error<B>> {
        if let Some(sensors_collection) = &self.data.sensors {
            let sc = sensors_collection
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            let mut sensor_data = Vec::with_capacity(sc.members.len());
            for sensor in &sc.members {
                sensor_data.push(SensorRef::new(
                    self.bmc.clone(),
                    NavProperty::<SchemaSensor>::new_reference(sensor.id().clone()),
                ));
            }
            Ok(sensor_data)
        } else {
            Err(Error::SensorsNotAvailable)
        }
    }

    /// Get `PCIe` devices for this computer system.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The systems does not have / provide pcie devices
    /// - Fetching pcie devices data fails
    #[cfg(feature = "pcie-devices")]
    pub async fn pcie_devices(&self) -> Result<PcieDeviceCollection<B>, crate::Error<B>> {
        let p = self
            .data
            .pcie_devices
            .as_ref()
            .ok_or(crate::Error::PcieDevicesNotAvailable)?;
        PcieDeviceCollection::new(&self.bmc, p).await
    }

    /// NVIDIA Bluefield OEM extension
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `Error::NvidiaChassisNotAvailable` if the systems does not have / provide NVIDIA OEM extension
    /// - Fetching data fails
    #[cfg(feature = "oem-nvidia-baseboard")]
    pub fn oem_nvidia_baseboard_cbc(&self) -> Result<NvidiaCbcChassis<B>, Error<B>> {
        self.data
            .base
            .base
            .oem
            .as_ref()
            .ok_or(Error::NvidiaCbcChassisNotAvailable)
            .and_then(NvidiaCbcChassis::new)
    }
}

impl<B: Bmc> Resource for Chassis<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}

fn remove_invalid_contained_by_fields(mut v: JsonValue) -> JsonValue {
    if let JsonValue::Object(ref mut obj) = v {
        if let Some(JsonValue::Object(ref mut links_obj)) = obj.get_mut("Links") {
            if let Some(JsonValue::Object(ref mut contained_by_obj)) =
                links_obj.get_mut("ContainedBy")
            {
                contained_by_obj.retain(|k, _| k == "@odata.id");
            }
        }
    }
    v
}
