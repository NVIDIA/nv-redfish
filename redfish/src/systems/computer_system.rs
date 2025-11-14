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

use crate::schema::redfish::computer_system::ComputerSystem as ComputerSystemSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "log-services")]
use crate::log_services::LogService;
#[cfg(feature = "memory")]
use crate::systems::Memory;
#[cfg(feature = "processors")]
use crate::systems::Processor;
#[cfg(feature = "storages")]
use crate::systems::Storage;

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
}

impl<B: Bmc> Resource for ComputerSystem<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
