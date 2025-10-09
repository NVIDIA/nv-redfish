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

use crate::schema::redfish::service_root::ServiceRoot as SchemaServiceRoot;
use crate::Error;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataId;
use std::sync::Arc;

#[cfg(feature = "accounts")]
use crate::accounts::AccountService;

/// Represents `ServiceRoot` in the BMC model.
pub struct ServiceRoot<B: Bmc> {
    root: Arc<SchemaServiceRoot>,
    bmc: Arc<B>,
}

impl<B: Bmc> Clone for ServiceRoot<B> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            bmc: self.bmc.clone(),
        }
    }
}

impl<B: Bmc> ServiceRoot<B> {
    /// Create a new service root.
    ///
    /// # Errors
    ///
    /// Returns error if retrieving the root path via Redfish fails.
    pub async fn new(bmc: Arc<B>) -> Result<Self, Error<B>> {
        let root = NavProperty::<SchemaServiceRoot>::new_reference(ODataId::service_root())
            .get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;
        Ok(Self {
            root,
            bmc: bmc.clone(),
        })
    }

    /// Get the account service belonging to the BMC.
    ///
    /// # Errors
    ///
    /// Returns error if retrieving account service data fails.
    #[cfg(feature = "accounts")]
    pub async fn account_service(&self) -> Result<AccountService<B>, Error<B>> {
        let service = self
            .root
            .account_service
            .as_ref()
            .ok_or(Error::AccountServiceNotSupported)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;
        Ok(AccountService::new(self, service, self.bmc.clone()))
    }
}

// Known Redfish implementation bug checks.
impl<B: Bmc> ServiceRoot<B> {
    // Account type is required according to schema specification
    // (marked with Redfish.Required annotation) but some vendors
    // ignores this flag. A workaround for this bug is supported by
    // `nv-redfish`.
    #[cfg(feature = "accounts")]
    pub(crate) fn bug_no_account_type_in_accounts(&self) -> bool {
        self.root.vendor.as_ref().is_some_and(|v| v == "HPE")
    }
}
