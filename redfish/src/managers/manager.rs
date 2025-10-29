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

use nv_redfish_core::Bmc;

use crate::schema::redfish::manager::Manager as ManagerSchema;

#[cfg(feature = "__log-service")]
use crate::log_service::LogService;

/// Represents a manager (BMC) in the system.
///
/// Provides access to manager information and associated services.
pub struct Manager<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<ManagerSchema>,
}

impl<B: Bmc + Sync + Send> Manager<B> {
    /// Create a new manager handle.
    pub(crate) const fn new(bmc: Arc<B>, data: Arc<ManagerSchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this manager.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<ManagerSchema> {
        self.data.clone()
    }

    /// Get log services for this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager does not have log services
    /// - Fetching log service data fails
    #[cfg(feature = "__log-service")]
    pub async fn list_log_services(&self) -> Result<Vec<LogService<B>>, crate::Error<B>> {
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
        for log_service_ref in &log_services_collection.members {
            let log_service = log_service_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(crate::Error::Bmc)?;
            log_services.push(LogService::new(self.bmc.clone(), log_service));
        }

        Ok(log_services)
    }
}
