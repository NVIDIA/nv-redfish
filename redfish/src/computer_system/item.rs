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
use crate::resource::PowerState;
use crate::schema::redfish::computer_system::ComputerSystem as ComputerSystemSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::convert::identity;
use std::sync::Arc;
use tagged_types::TaggedType;

#[cfg(feature = "boot-options")]
use crate::computer_system::BootOptionCollection;
#[cfg(feature = "memory")]
use crate::computer_system::Memory;
#[cfg(feature = "processors")]
use crate::computer_system::Processor;
#[cfg(feature = "storages")]
use crate::computer_system::Storage;
#[cfg(feature = "ethernet-interfaces")]
use crate::ethernet_interface::EthernetInterfaceCollection;
#[cfg(feature = "log-services")]
use crate::log_service::LogService;

#[doc(hidden)]
pub enum ComputerSystemTag {}

/// Computer system manufacturer.
pub type Manufacturer<T> = HardwareIdManufacturer<T, ComputerSystemTag>;

/// Computer system model.
pub type Model<T> = HardwareIdModel<T, ComputerSystemTag>;

/// Computer system part number.
pub type PartNumber<T> = HardwareIdPartNumber<T, ComputerSystemTag>;

/// Computer system serial number.
pub type SerialNumber<T> = HardwareIdSerialNumber<T, ComputerSystemTag>;

/// Computer system SKU.
pub type Sku<T> = TaggedType<T, ComputerSystemSkuTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ComputerSystemSkuTag {}

/// `BootOptionReference` type represent boot order of the `ComputerSystem`.
pub type BootOptionReference<T> = TaggedType<T, BootOptionReferenceTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum BootOptionReferenceTag {}

/// Represents a computer system in the BMC.
///
/// Provides access to system information and sub-resources such as processors.
pub struct ComputerSystem<B: Bmc> {
    #[allow(dead_code)] // feature-enabled...
    bmc: NvBmc<B>,
    data: Arc<ComputerSystemSchema>,
}

impl<B: Bmc> ComputerSystem<B> {
    /// Create a new computer system handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ComputerSystemSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    /// Get the raw schema data for this computer system.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<ComputerSystemSchema> {
        self.data.clone()
    }
    /// Get hardware identifier of the network adpater.
    #[must_use]
    pub fn hardware_id(&self) -> HardwareIdRef<'_, ComputerSystemTag> {
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

    /// The manufacturer SKU for this system.
    #[must_use]
    pub fn sku(&self) -> Option<Sku<&String>> {
        self.data
            .sku
            .as_ref()
            .and_then(Option::as_ref)
            .map(Sku::new)
    }

    /// Power state of this system.
    #[must_use]
    pub fn power_state(&self) -> Option<PowerState> {
        self.data.power_state.and_then(identity)
    }

    /// An array of `BootOptionReference` strings that represent the persistent boot order for with this
    /// computer system.
    #[must_use]
    pub fn boot_order(&self) -> Option<Vec<BootOptionReference<&String>>> {
        self.data
            .as_ref()
            .boot
            .as_ref()
            .and_then(|boot| boot.boot_order.as_ref().and_then(Option::as_ref))
            .map(|v| v.iter().map(BootOptionReference::new).collect::<Vec<_>>())
    }

    /// Get processors associated with this system.
    ///
    /// Fetches the processor collection and returns a list of [`Processor`] handles.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The system does not have a processors collection
    /// - Fetching processor data fails
    #[cfg(feature = "processors")]
    pub async fn processors(&self) -> Result<Vec<Processor<B>>, Error<B>> {
        let processors_ref = self
            .data
            .processors
            .as_ref()
            .ok_or(Error::ProcessorsNotAvailable)?;

        let processors_collection = self.bmc.expand_property(processors_ref).await?;

        let mut processors = Vec::new();
        for m in &processors_collection.members {
            processors.push(Processor::new(&self.bmc, m).await?);
        }

        Ok(processors)
    }

    /// Get storage controllers associated with this system.
    ///
    /// Fetches the storage collection and returns a list of [`Storage`] handles.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The system does not have a storage collection
    /// - Fetching storage data fails
    #[cfg(feature = "storages")]
    pub async fn storage_controllers(&self) -> Result<Vec<Storage<B>>, Error<B>> {
        let storage_ref = self
            .data
            .storage
            .as_ref()
            .ok_or(Error::StorageNotAvailable)?;

        let storage_collection = self.bmc.expand_property(storage_ref).await?;

        let mut storage_controllers = Vec::new();
        for m in &storage_collection.members {
            storage_controllers.push(Storage::new(&self.bmc, m).await?);
        }

        Ok(storage_controllers)
    }

    /// Get memory modules associated with this system.
    ///
    /// Fetches the memory collection and returns a list of [`Memory`] handles.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The system does not have a memory collection
    /// - Fetching memory data fails
    #[cfg(feature = "memory")]
    pub async fn memory_modules(&self) -> Result<Vec<Memory<B>>, Error<B>> {
        let memory_ref = self.data.memory.as_ref().ok_or(Error::MemoryNotAvailable)?;

        let memory_collection = self.bmc.expand_property(memory_ref).await?;

        let mut memory_modules = Vec::new();
        for m in &memory_collection.members {
            memory_modules.push(Memory::new(&self.bmc, m).await?);
        }

        Ok(memory_modules)
    }

    /// Get log services for this computer system.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The computer system does not have log services
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

    /// Get ethernet interfaces for this computer system.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The sytems does not have / provide ethernet interfaces
    /// - Fetching ethernet internet data fails
    #[cfg(feature = "ethernet-interfaces")]
    pub async fn ethernet_interfaces(
        &self,
    ) -> Result<EthernetInterfaceCollection<B>, crate::Error<B>> {
        let p = self
            .data
            .ethernet_interfaces
            .as_ref()
            .ok_or(crate::Error::EthernetInterfacesNotAvailable)?;
        EthernetInterfaceCollection::new(&self.bmc, p).await
    }

    /// Get collection of the UEFI boot options associated with this computer system.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The sytems does not have / provide boot options
    /// - Fetching boot options data fails
    #[cfg(feature = "boot-options")]
    pub async fn boot_options(&self) -> Result<BootOptionCollection<B>, crate::Error<B>> {
        let p = self
            .data
            .boot
            .as_ref()
            .ok_or(crate::Error::BootOptionsNotAvailable)?
            .boot_options
            .as_ref()
            .ok_or(crate::Error::BootOptionsNotAvailable)?;
        BootOptionCollection::new(&self.bmc, p).await
    }
}

impl<B: Bmc> Resource for ComputerSystem<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
