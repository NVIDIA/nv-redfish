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
#[cfg(feature = "accounts")]
use crate::accounts::SlotDefinedConfig as SlotDefinedUserAccountsConfig;
#[cfg(feature = "chassis")]
use crate::chassis::ChassisCollection;
#[cfg(feature = "managers")]
use crate::managers::ManagerCollection;
#[cfg(feature = "systems")]
use crate::systems::SystemCollection;
#[cfg(feature = "update-service")]
use crate::update_service::UpdateService;

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

    /// Get chassis collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if chassis list is not avaiable in BMC
    #[cfg(feature = "chassis")]
    pub async fn chassis_collection(&self) -> Result<ChassisCollection<B>, Error<B>> {
        let chassis = self
            .root
            .chassis
            .as_ref()
            .ok_or(Error::ChassisNotSupported)?;
        ChassisCollection::new(self.bmc.clone(), chassis).await
    }

    /// Get computer system collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if system list is not available in BMC
    #[cfg(feature = "systems")]
    pub async fn system_collection(&self) -> Result<SystemCollection<B>, Error<B>> {
        let systems = self
            .root
            .systems
            .as_ref()
            .ok_or(Error::SystemNotSupported)?;
        SystemCollection::new(self.bmc.clone(), systems).await
    }

    /// Get update service in BMC
    ///
    /// # Errors
    ///
    /// Returns error if update service is not available in BMC
    #[cfg(feature = "update-service")]
    pub async fn update_service(&self) -> Result<UpdateService<B>, Error<B>> {
        let service_ref = self
            .root
            .update_service
            .as_ref()
            .ok_or(Error::UpdateServiceNotSupported)?;
        let service = service_ref
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;
        Ok(UpdateService::new(self.bmc.clone(), service))
    }

    /// Get manager collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if manager list is not available in BMC
    #[cfg(feature = "managers")]
    pub async fn list_managers(&self) -> Result<ManagerCollection<B>, Error<B>> {
        let managers = self
            .root
            .managers
            .as_ref()
            .ok_or(Error::ManagerNotSupported)?;
        ManagerCollection::new(self.bmc.clone(), managers).await
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
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "HPE")
    }

    // In some implementations BMC cannot create / delete Redfish
    // accounts but have pre-created accounts (slots). Workflow is as
    // following: to "create" new account user should update
    // precreated account with new parameters and enable it. To delete
    // account user should just disable it.
    #[cfg(feature = "accounts")]
    pub(crate) fn slot_defined_user_accounts(&self) -> Option<SlotDefinedUserAccountsConfig> {
        if self
            .root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "Dell")
        {
            Some(SlotDefinedUserAccountsConfig {
                min_slot: Some(3),
                hide_disabled: true,
                disable_account_on_delete: true,
            })
        } else {
            None
        }
    }
}
