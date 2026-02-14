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

use nv_redfish_core::{Bmc, NavProperty, ODataId};
use tagged_types::TaggedType;

#[cfg(feature = "accounts")]
use crate::account::AccountService;
#[cfg(feature = "accounts")]
use crate::account::SlotDefinedConfig as SlotDefinedUserAccountsConfig;
#[cfg(feature = "chassis")]
use crate::chassis::ChassisCollection;
#[cfg(feature = "computer-systems")]
use crate::computer_system::SystemCollection;
#[cfg(feature = "managers")]
use crate::manager::ManagerCollection;
use crate::schema::redfish::service_root::ServiceRoot as SchemaServiceRoot;
#[cfg(feature = "update-service")]
use crate::update_service::UpdateService;
use crate::{Error, NvBmc, ProtocolFeatures, Resource, ResourceSchema};

/// The vendor or manufacturer associated with Redfish service.
pub type Vendor<T> = TaggedType<T, VendorTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum VendorTag {}

/// The product associated with Redfish service..
pub type Product<T> = TaggedType<T, ProductTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ProductTag {}

/// Represents `ServiceRoot` in the BMC model.
#[derive(Clone)]
pub struct ServiceRoot<B: Bmc> {
    /// Content of the root.
    pub root: Arc<SchemaServiceRoot>,
    #[allow(dead_code)] // feature-enabled field
    bmc: NvBmc<B>,
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
        let mut protocol_features = root
            .protocol_features_supported
            .as_ref()
            .map(ProtocolFeatures::new)
            .unwrap_or_default();

        if Self::expand_is_not_working_properly(&root) {
            protocol_features.expand.expand_all = false;
            protocol_features.expand.no_links = false;
        }

        let bmc = NvBmc::new(bmc, protocol_features);
        Ok(Self { root, bmc })
    }

    /// The vendor or manufacturer associated with this Redfish service.
    pub fn vendor(&self) -> Option<Vendor<&String>> {
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .map(Vendor::new)
    }

    /// The product associated with this Redfish service.
    pub fn product(&self) -> Option<Product<&String>> {
        self.root
            .product
            .as_ref()
            .and_then(Option::as_ref)
            .map(Product::new)
    }

    /// Get the account service belonging to the BMC.
    ///
    /// # Errors
    ///
    /// Returns error if retrieving account service data fails.
    #[cfg(feature = "accounts")]
    pub async fn account_service(&self) -> Result<AccountService<B>, Error<B>> {
        AccountService::new(&self.bmc, self).await
    }

    /// Get chassis collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if chassis list is not avaiable in BMC
    #[cfg(feature = "chassis")]
    pub async fn chassis(&self) -> Result<ChassisCollection<B>, Error<B>> {
        ChassisCollection::new(&self.bmc, self).await
    }

    /// Get computer system collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if system list is not available in BMC
    #[cfg(feature = "computer-systems")]
    pub async fn systems(&self) -> Result<SystemCollection<B>, Error<B>> {
        SystemCollection::new(&self.bmc, self).await
    }

    /// Get update service in BMC
    ///
    /// # Errors
    ///
    /// Returns error if update service is not available in BMC
    #[cfg(feature = "update-service")]
    pub async fn update_service(&self) -> Result<UpdateService<B>, Error<B>> {
        UpdateService::new(&self.bmc, self).await
    }

    /// Get manager collection in BMC
    ///
    /// # Errors
    ///
    /// Returns error if manager list is not available in BMC
    #[cfg(feature = "managers")]
    pub async fn managers(&self) -> Result<ManagerCollection<B>, Error<B>> {
        ManagerCollection::new(&self.bmc, self).await
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

    // In some implementations BMC ReleaseDate is incorrectly set to
    // 00:00:00Z in FirmwareInventory (which is
    // SoftwareInventoryCollection).
    #[cfg(feature = "update-service")]
    pub(crate) fn fw_inventory_wrong_release_date(&self) -> bool {
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "Dell")
    }

    /// In some cases thre is addtional fields in Links.ContainedBy in
    /// Chassis resource, this flag aims to patch this invalid links
    #[cfg(feature = "chassis")]
    pub(crate) fn bug_invalid_contained_by_fields(&self) -> bool {
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "AMI")
            && self
                .root
                .redfish_version
                .as_ref()
                .is_some_and(|version| version == "1.11.0")
    }

    /// In some implementations BMC ReleaseDate is incorrectly set to
    /// "0000-00-00T00:00:00+00:00" in ComputerSystem/LastResetTime
    /// This prevents ComputerSystem to be correctly parsed because
    /// this is invalid Edm.DateTimeOffset.
    #[cfg(feature = "computer-systems")]
    pub(crate) fn computer_systems_wrong_last_reset_time(&self) -> bool {
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "Dell")
    }

    /// In some implementations Assembly/Assemblies doens't
    /// "@odata.type". This is spec violation but we support patching
    /// such systems.
    #[cfg(feature = "assembly")]
    pub(crate) fn assembly_assemblies_without_odata_type(&self) -> bool {
        self.root
            .vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "WIWYNN")
    }

    /// In some cases we expand is not working according to spec,
    /// if it is the case for specific chassis, we would disable
    /// expand api
    fn expand_is_not_working_properly(root: &SchemaServiceRoot) -> bool {
        root.vendor
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|v| v == "AMI")
            && root
                .redfish_version
                .as_ref()
                .is_some_and(|version| version == "1.11.0")
    }
}

impl<B: Bmc> Resource for ServiceRoot<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.root.as_ref().base
    }
}
