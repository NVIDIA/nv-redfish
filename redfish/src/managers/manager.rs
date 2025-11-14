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
use crate::ethernet_interfaces::EthernetInterfaceCollection;
#[cfg(feature = "log-services")]
use crate::log_services::LogService;

/// Represents a manager (BMC) in the system.
///
/// Provides access to manager information and associated services.
pub struct Manager<B: Bmc> {
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
    /// - Fetching log ethernet internet data fails
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
}

impl<B: Bmc> Resource for Manager<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
