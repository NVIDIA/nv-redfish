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
#[cfg(feature = "oem-hpe")]
use crate::oem::hpe::manager::HpeManager;
#[cfg(feature = "oem-lenovo")]
use crate::oem::lenovo::manager::LenovoManager;
#[cfg(feature = "oem-supermicro")]
use crate::oem::supermicro::manager::SupermicroManager;

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
    /// Returns `Ok(None)` when the ethernet interfaces link is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching ethernet interfaces data fails.
    #[cfg(feature = "ethernet-interfaces")]
    pub async fn ethernet_interfaces(
        &self,
    ) -> Result<Option<EthernetInterfaceCollection<B>>, crate::Error<B>> {
        if let Some(p) = &self.data.ethernet_interfaces {
            EthernetInterfaceCollection::new(&self.bmc, p)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    }

    /// Get host interfaces for this manager.
    ///
    /// Returns `Ok(None)` when the host interfaces link is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching host interfaces data fails.
    #[cfg(feature = "host-interfaces")]
    pub async fn host_interfaces(
        &self,
    ) -> Result<Option<HostInterfaceCollection<B>>, crate::Error<B>> {
        if let Some(p) = &self.data.host_interfaces {
            HostInterfaceCollection::new(&self.bmc, p).await.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Get log services for this manager.
    ///
    /// Returns `Ok(None)` when the log services link is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching log service data fails.
    #[cfg(feature = "log-services")]
    pub async fn log_services(&self) -> Result<Option<Vec<LogService<B>>>, crate::Error<B>> {
        if let Some(log_services_ref) = &self.data.log_services {
            let log_services_collection = log_services_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(crate::Error::Bmc)?;

            let mut log_services = Vec::new();
            for m in &log_services_collection.members {
                log_services.push(LogService::new(&self.bmc, m).await?);
            }

            Ok(Some(log_services))
        } else {
            Ok(None)
        }
    }

    /// Get Dell Manager attributes for this manager.
    ///
    /// Returns `Ok(None)` when the manager does not include `Oem.Dell`.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching manager attributes data fails.
    #[cfg(feature = "oem-dell-attributes")]
    pub async fn oem_dell_attributes(&self) -> Result<Option<DellAttributes<B>>, Error<B>> {
        DellAttributes::manager_attributes(&self.bmc, &self.data).await
    }

    /// Get Lenovo Manager OEM.
    ///
    /// Returns `Ok(None)` when the manager does not include `Oem.Lenovo`.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing Lenovo manager OEM data fails.
    #[cfg(feature = "oem-lenovo")]
    pub fn oem_lenovo(&self) -> Result<Option<LenovoManager<B>>, Error<B>> {
        LenovoManager::new(&self.bmc, &self.data)
    }

    /// Get HPE Manager OEM.
    ///
    /// Returns `Ok(None)` when the manager does not include `Oem.Hpe`.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing HPE manager OEM data fails.
    #[cfg(feature = "oem-hpe")]
    pub fn oem_hpe(&self) -> Result<Option<HpeManager<B>>, Error<B>> {
        HpeManager::new(&self.bmc, &self.data)
    }

    /// Get Supermicro Manager OEM.
    ///
    /// Returns `Ok(None)` when the manager does not include `Oem.Supermicro`.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing Supermicro manager OEM data fails.
    #[cfg(feature = "oem-supermicro")]
    pub fn oem_supermicro(&self) -> Result<Option<SupermicroManager<B>>, Error<B>> {
        SupermicroManager::new(&self.bmc, &self.data)
    }
}

impl<B: Bmc> Resource for Manager<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
