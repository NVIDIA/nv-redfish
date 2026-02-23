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

use crate::schema::redfish::manager::Manager as ManagerSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[cfg(feature = "ethernet-interfaces")]
use crate::ethernet_interface::EthernetInterfaceCollection;
#[cfg(feature = "host-interfaces")]
use crate::host_interface::HostInterfaceCollection;
#[cfg(feature = "log-services")]
use crate::log_service::LogService;
#[cfg(feature = "oem-dell-attributes")]
use crate::oem::dell::attributes::DellAttributes;
#[cfg(feature = "oem-lenovo")]
use crate::oem::lenovo::manager::LenovoManager;

/// Represents a manager (BMC) in the system.
///
/// Provides access to manager information and associated services.
pub struct Manager<B: Bmc> {
    #[allow(dead_code)] // enabled by features
    bmc: NvBmc<B>,
    data: Arc<ManagerSchema>,
}

impl<B: Bmc> Manager<B> {
    /// Create a new manager handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ManagerSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    /// Get the raw schema data for this manager.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<ManagerSchema> {
        self.data.clone()
    }

    /// Get ethernet interfaces for this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have / provide ethernet interfaces
    /// - Fetching ethernet interfaces data fails
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

    /// Get host interfaces for this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have / provide host interfaces
    /// - Fetching host interfaces data fails
    #[cfg(feature = "host-interfaces")]
    pub async fn host_interfaces(&self) -> Result<HostInterfaceCollection<B>, crate::Error<B>> {
        let p = self
            .data
            .host_interfaces
            .as_ref()
            .ok_or(crate::Error::HostInterfacesNotAvailable)?;
        HostInterfaceCollection::new(&self.bmc, p).await
    }

    /// Get log services for this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have log services
    /// - Fetching log service data fails
    #[cfg(feature = "log-services")]
    pub async fn log_services(&self) -> Result<Vec<LogService<B>>, crate::Error<B>> {
        let log_services_ref = self
            .data
            .log_services
            .as_ref()
            .ok_or(crate::Error::LogServiceNotAvailable)?;

        let log_services_collection = log_services_ref
            .get(self.bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)?;

        let mut log_services = Vec::new();
        for m in &log_services_collection.members {
            log_services.push(LogService::new(&self.bmc, m).await?);
        }

        Ok(log_services)
    }

    /// Get Dell Manager attributes for this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have dell attributes (not a Dell)
    /// - Fetching manager attributes data fails
    #[cfg(feature = "oem-dell-attributes")]
    pub async fn oem_dell_attributes(&self) -> Result<DellAttributes<B>, Error<B>> {
        DellAttributes::manager_attributes(&self.bmc, &self.data).await
    }

    /// Get Lenovo Manager OEM.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have Oem/Lenovo (not a Lenovo)
    /// - Fetching manager attributes data fails
    #[cfg(feature = "oem-lenovo")]
    pub fn oem_lenovo(&self) -> Result<LenovoManager<B>, Error<B>> {
        LenovoManager::new(&self.bmc, &self.data)
    }
}

impl<B: Bmc> Resource for Manager<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
