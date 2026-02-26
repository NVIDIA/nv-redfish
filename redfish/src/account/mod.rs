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

//! AccountService (Redfish) — high-level wrappers
//!
//! Feature: `accounts` (this module is compiled only when the feature is enabled).
//!
//! This module provides ergonomic wrappers around the generated Redfish
//! AccountService model:
//! - `AccountService`: entry point to manage accounts
//! - `AccountCollection`: access and create `ManagerAccount` members
//! - `Account`: operate on an individual `ManagerAccount`
//!
//! Vendor compatibility
//! - Some implementations omit fields marked as `Redfish.Required`.
//! - This crate can apply read/response patches (see `patch_support`) to keep
//!   behavior compatible across vendors (for example, defaulting `AccountTypes`).
//!

/// Collection of accounts.
mod collection;
/// Account inside account service.
mod item;

use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::account_service::AccountService as SchemaAccountService;
use crate::Error;
use crate::NvBmc;
use crate::ServiceRoot;
use nv_redfish_core::Bmc;
use std::sync::Arc;

#[doc(inline)]
pub use crate::schema::redfish::manager_account::AccountTypes;
#[doc(inline)]
pub use crate::schema::redfish::manager_account::ManagerAccountCreate;
#[doc(inline)]
pub use crate::schema::redfish::manager_account::ManagerAccountUpdate;
#[doc(inline)]
pub use item::Account;

#[doc(inline)]
pub use collection::AccountCollection;
#[doc(inline)]
pub(crate) use collection::SlotDefinedConfig;
#[doc(inline)]
pub(crate) use item::Config as AccountConfig;

/// Account service. Provides the ability to manage accounts via Redfish.
pub struct AccountService<B: Bmc> {
    collection_config: collection::Config,
    service: Arc<SchemaAccountService>,
    bmc: NvBmc<B>,
}

impl<B: Bmc> AccountService<B> {
    /// Create a new account service. This is always done by
    /// `ServiceRoot` object.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        root: &ServiceRoot<B>,
    ) -> Result<Option<Self>, Error<B>> {
        let Some(service_nav) = root.root.account_service.as_ref() else {
            return Ok(None);
        };
        let service = if root.bug_null_in_remote_role_mapping() {
            Payload::get(bmc.as_ref(), service_nav, remove_nulls_from_account).await?
        } else {
            service_nav.get(bmc.as_ref()).await.map_err(Error::Bmc)?
        };

        let mut patches = Vec::new();
        if root.bug_no_account_type_in_accounts() {
            patches.push(append_default_account_type);
        }
        let account_read_patch_fn = if patches.is_empty() {
            None
        } else {
            let account_read_patch_fn: ReadPatchFn =
                Arc::new(move |v| patches.iter().fold(v, |acc, f| f(acc)));
            Some(account_read_patch_fn)
        };
        let slot_defined_user_accounts = root.slot_defined_user_accounts();
        Ok(Some(Self {
            collection_config: collection::Config {
                account: AccountConfig {
                    read_patch_fn: account_read_patch_fn,
                    disable_account_on_delete: slot_defined_user_accounts
                        .as_ref()
                        .is_some_and(|cfg| cfg.disable_account_on_delete),
                },
                slot_defined_user_accounts,
            },
            service,
            bmc: bmc.clone(),
        }))
    }

    /// Get the raw schema data for this account service.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<SchemaAccountService> {
        self.service.clone()
    }

    /// Get the accounts collection.
    ///
    /// Uses `$expand` to retrieve members in a single request when supported.
    ///
    /// # Errors
    ///
    /// Returns an error if expanding the collection fails.
    pub async fn accounts(&self) -> Result<Option<AccountCollection<B>>, Error<B>> {
        if let Some(collection_ref) = self.service.accounts.as_ref() {
            AccountCollection::new(
                self.bmc.clone(),
                collection_ref,
                self.collection_config.clone(),
            )
            .await
            .map(Some)
        } else {
            Ok(None)
        }
    }
}

// `AccountTypes` is marked as `Redfish.Required`, but some systems
// ignore this requirement. The account service replaces its value with
// a reasonable default (see below).
//
// Note quote from schema: "if this property is not provided by the client, the default value
// shall be an array that contains the value `Redfish`".
fn append_default_account_type(v: JsonValue) -> JsonValue {
    if let JsonValue::Object(mut obj) = v {
        obj.entry("AccountTypes")
            .or_insert(JsonValue::Array(vec![JsonValue::String("Redfish".into())]));
        JsonValue::Object(obj)
    } else {
        v
    }
}

fn remove_nulls_from_account(v: JsonValue) -> JsonValue {
    fn patch_external_account_provider(provider: &mut JsonValue) {
        if let JsonValue::Object(provider_obj) = provider {
            if let Some(JsonValue::Array(mapping)) = provider_obj.get_mut("RemoteRoleMapping") {
                mapping.retain(|entry| !entry.is_null());
            }
        }
    }

    if let JsonValue::Object(mut obj) = v {
        for provider in ["ActiveDirectory", "LDAP", "TACACSplus", "OAuth2"] {
            if let Some(provider_obj) = obj.get_mut(provider) {
                patch_external_account_provider(provider_obj);
            }
        }
        JsonValue::Object(obj)
    } else {
        v
    }
}
