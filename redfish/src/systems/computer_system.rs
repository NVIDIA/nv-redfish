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
use crate::systems::Memory;
use crate::systems::Processor;
use crate::systems::Storage;
use crate::Error;
use nv_redfish_core::http::ExpandQuery;
use nv_redfish_core::Bmc;
use nv_redfish_core::Expandable as _;
use std::sync::Arc;

#[cfg(feature = "__log-service")]
use crate::log_service::LogService;

/// Represents a computer system in the BMC.
///
/// Provides access to system information and sub-resources such as processors.
pub struct ComputerSystem<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<ComputerSystemSchema>,
}

impl<B> ComputerSystem<B>
where
    B: Bmc + Sync + Send,
{
    /// Create a new computer system handle.
    pub(crate) const fn new(bmc: Arc<B>, data: Arc<ComputerSystemSchema>) -> Self {
        Self { bmc, data }
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
    pub async fn get_processors(&self) -> Result<Vec<Processor<B>>, Error<B>> {
        let processors_ref = self
            .data
            .processors
            .as_ref()
            .ok_or(Error::ProcessorsNotAvailable)?;

        let processors_collection = processors_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut processors = Vec::new();
        for processor_ref in &processors_collection.members {
            let processor = processor_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            processors.push(Processor::new(self.bmc.clone(), processor));
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
    pub async fn get_storage(&self) -> Result<Vec<Storage<B>>, Error<B>> {
        let storage_ref = self
            .data
            .storage
            .as_ref()
            .ok_or(Error::StorageNotAvailable)?;

        let storage_collection = storage_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut storage_controllers = Vec::new();
        for storage_controller_ref in &storage_collection.members {
            let storage_controller = storage_controller_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            storage_controllers.push(Storage::new(self.bmc.clone(), storage_controller));
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
    pub async fn get_memory_modules(&self) -> Result<Vec<Memory<B>>, Error<B>> {
        let memory_ref = self.data.memory.as_ref().ok_or(Error::MemoryNotAvailable)?;

        let memory_collection = memory_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut memory_modules = Vec::new();
        for memory_ref in &memory_collection.members {
            let memory = memory_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            memory_modules.push(Memory::new(self.bmc.clone(), memory));
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
    #[cfg(feature = "__log-service")]
    pub async fn list_log_services(&self) -> Result<Vec<LogService<B>>, Error<B>> {
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
            log_services.push(LogService::new(self.bmc.clone(), log_service));
        }

        Ok(log_services)
    }
}
