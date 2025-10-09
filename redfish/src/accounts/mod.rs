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

use crate::patch_support::CollectionWithPatch;
use crate::patch_support::CreateWithPatch;
use crate::patch_support::JsonValue;
use crate::patch_support::ReadPatchFn;
use crate::patch_support::UpdateWithPatch;
use crate::schema::redfish::account_service::AccountService as SchemaAccountService;
use crate::schema::redfish::manager_account::ManagerAccount;
use crate::schema::redfish::manager_account_collection::ManagerAccountCollection;
use crate::schema::redfish::resource::ResourceCollection;
use crate::Error;
use crate::ServiceRoot;
use nv_redfish_core::http::ExpandQuery;
use nv_redfish_core::Bmc;
use nv_redfish_core::Deletable as _;
use nv_redfish_core::EntityTypeRef as _;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataId;
use std::sync::Arc;

#[doc(inline)]
pub use crate::schema::redfish::manager_account::AccountTypes;
#[doc(inline)]
pub use crate::schema::redfish::manager_account::ManagerAccountCreate;
#[doc(inline)]
pub use crate::schema::redfish::manager_account::ManagerAccountUpdate;

/// Account service. Provides the ability to manage accounts via Redfish.
pub struct AccountService<B: Bmc> {
    account_read_patch_fn: Option<ReadPatchFn>,
    service: Arc<SchemaAccountService>,
    bmc: Arc<B>,
}

impl<B: Bmc> AccountService<B> {
    /// Create a new account service. This is always done by
    /// `ServiceRoot` object.
    pub(crate) fn new(
        root: &ServiceRoot<B>,
        service: Arc<SchemaAccountService>,
        bmc: Arc<B>,
    ) -> Self {
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

        Self {
            account_read_patch_fn,
            service,
            bmc,
        }
    }

    /// `OData` identifier of the `AccountService` in Redfish.
    ///
    /// It is almost always `/redfish/v1/AccountService`.
    #[must_use]
    pub fn odata_id(&self) -> &ODataId {
        self.service.as_ref().id()
    }

    /// Get accounts collection.
    ///
    /// Note that it tries to use `expand` to get all members of the
    /// collection in one request.
    ///
    /// # Errors
    ///
    /// Returns error if expanding the collection fails.
    pub async fn accounts(&self) -> Result<AccountCollection<B>, Error<B>> {
        let collection_ref = self
            .service
            .accounts
            .as_ref()
            .ok_or(Error::AccountServiceNotSupported)?;

        let query = ExpandQuery::default().levels(1);
        Ok(AccountCollection {
            read_patch_fn: self.account_read_patch_fn.clone(),
            bmc: self.bmc.clone(),
            collection: AccountCollection::read_collection(
                self.bmc.as_ref(),
                collection_ref,
                self.account_read_patch_fn.as_ref(),
                query,
            )
            .await?,
        })
    }
}

/// Accounts collection.
///
/// Provides functions to access collection members.
pub struct AccountCollection<B: Bmc> {
    read_patch_fn: Option<ReadPatchFn>,
    bmc: Arc<B>,
    collection: Arc<ManagerAccountCollection>,
}

impl<B: Bmc> CollectionWithPatch<ManagerAccountCollection, ManagerAccount, B>
    for AccountCollection<B>
{
    fn convert_patched(
        base: ResourceCollection,
        members: Vec<NavProperty<ManagerAccount>>,
    ) -> ManagerAccountCollection {
        ManagerAccountCollection { base, members }
    }
}

impl<B: Bmc + Sync + Send>
    CreateWithPatch<ManagerAccountCollection, ManagerAccount, ManagerAccountCreate, B>
    for AccountCollection<B>
{
    fn entity_ref(&self) -> &ManagerAccountCollection {
        self.collection.as_ref()
    }
    fn patch(&self) -> Option<&ReadPatchFn> {
        self.read_patch_fn.as_ref()
    }
    fn bmc(&self) -> &B {
        &self.bmc
    }
}

impl<B: Bmc + Sync + Send> AccountCollection<B> {
    /// `OData` identifier of the account collection in Redfish.
    ///
    /// It is almost always `/redfish/v1/AccountService/Accounts`.
    #[must_use]
    pub fn odata_id(&self) -> &ODataId {
        self.collection.as_ref().id()
    }

    /// Create a new account.
    ///
    /// # Errors
    ///
    /// Returns error if creating a new account fails.
    pub async fn create_account(
        &self,
        create: &ManagerAccountCreate,
    ) -> Result<Account<B>, Error<B>> {
        let account = self.create_with_patch(create).await?;
        Ok(Account {
            read_patch_fn: self.read_patch_fn.clone(),
            bmc: self.bmc.clone(),
            data: Arc::new(account),
        })
    }

    /// Get accounts data.
    ///
    /// This method doesn't update the collection itself. It only
    /// retrieves all account data (if not already retrieved).
    ///
    /// # Errors
    ///
    /// Returns error if getting account data fails. This only happens
    /// if the account collection wasn't expanded for some reason.
    pub async fn all_accounts_data(&self) -> Result<Vec<Account<B>>, Error<B>> {
        let mut result = Vec::with_capacity(self.collection.members.len());
        for m in &self.collection.members {
            result.push(Account {
                read_patch_fn: self.read_patch_fn.clone(),
                bmc: self.bmc.clone(),
                data: m.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?,
            });
        }
        Ok(result)
    }
}

/// Represents `ManagerAccount` in Redfish.
pub struct Account<B: Bmc> {
    read_patch_fn: Option<ReadPatchFn>,
    bmc: Arc<B>,
    data: Arc<ManagerAccount>,
}

impl<B> UpdateWithPatch<ManagerAccount, ManagerAccountUpdate, B> for Account<B>
where
    B: Bmc + Sync + Send,
{
    fn entity_ref(&self) -> &ManagerAccount {
        self.data.as_ref()
    }
    fn patch(&self) -> Option<&ReadPatchFn> {
        self.read_patch_fn.as_ref()
    }
    fn bmc(&self) -> &B {
        &self.bmc
    }
}

impl<B> Account<B>
where
    B: Bmc + Sync + Send,
{
    /// Raw data of the account.
    #[must_use]
    pub fn raw(&self) -> Arc<ManagerAccount> {
        self.data.clone()
    }

    /// Update the account using Redfish.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns error if the server returns an error or if the response
    /// fails to parse.
    pub async fn update(&self, update: &ManagerAccountUpdate) -> Result<Self, Error<B>> {
        let account = self.update_with_patch(update).await?;
        Ok(Self {
            read_patch_fn: self.read_patch_fn.clone(),
            bmc: self.bmc.clone(),
            data: Arc::new(account),
        })
    }

    /// Update the account password.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns error if the server returns an error or if the response
    /// fails to parse.
    pub async fn update_password(&self, password: String) -> Result<Self, Error<B>> {
        self.update(
            &ManagerAccountUpdate::builder()
                .with_password(password)
                .build(),
        )
        .await
    }

    /// Update the current account's user name.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns error if the server returns an error or if the response
    /// fails to parse.
    pub async fn update_user_name(&self, user_name: String) -> Result<Self, Error<B>> {
        self.update(
            &ManagerAccountUpdate::builder()
                .with_user_name(user_name)
                .build(),
        )
        .await
    }

    /// Delete the current account.
    ///
    /// # Errors
    ///
    /// Returns error if the server returns an error on delete.
    pub async fn delete(&self) -> Result<(), Error<B>> {
        self.data
            .delete(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|_| ())
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
