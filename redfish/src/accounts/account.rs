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

//! Redfish ManagerAccount — high-level wrapper
//!
//! Provides `Account`, an ergonomic handle over a Redfish `ManagerAccount`:
//! - Read raw data with `raw()`
//! - Update fields via `update()`, or use helpers `update_password()` and
//!   `update_user_name()`
//! - Delete the account with `delete()`; optionally disable instead of deleting
//!   when configured
//!
//! Configuration:
//! - `Config::read_patch_fn`: apply read-time JSON patches for vendor
//!   compatibility
//! - `Config::disable_account_on_delete`: make `delete()` disable the account
//!   rather than remove it
//!
//! Note: `Account` objects are created by higher-level APIs (e.g.
//! `AccountCollection`) and do not create accounts on the BMC by themselves.
//! Use the collection to create new accounts.

use crate::accounts::ManagerAccountUpdate;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::patch_support::UpdateWithPatch;
use crate::schema::redfish::manager_account::ManagerAccount;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::Deletable as _;
use nv_redfish_core::NavProperty;
use std::convert::identity;
use std::sync::Arc;

#[derive(Clone)]
pub struct Config {
    /// Function to patch input JSON when reading account structures.
    pub read_patch_fn: Option<ReadPatchFn>,
    /// If true, deletion disables the account instead of removing it.
    pub disable_account_on_delete: bool,
}

/// Represents a Redfish `ManagerAccount`.
pub struct Account<B: Bmc> {
    config: Config,
    bmc: NvBmc<B>,
    data: Arc<ManagerAccount>,
}

impl<B: Bmc> UpdateWithPatch<ManagerAccount, ManagerAccountUpdate, B> for Account<B> {
    fn entity_ref(&self) -> &ManagerAccount {
        self.data.as_ref()
    }
    fn patch(&self) -> Option<&ReadPatchFn> {
        self.config.read_patch_fn.as_ref()
    }
    fn bmc(&self) -> &B {
        self.bmc.as_ref()
    }
}

impl<B: Bmc> Account<B> {
    /// Create a new account handle. This does not create an account on the
    /// BMC.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ManagerAccount>,
        config: &Config,
    ) -> Result<Self, Error<B>> {
        if let Some(read_patch_fn) = &config.read_patch_fn {
            Payload::get(bmc.as_ref(), nav, read_patch_fn.as_ref()).await
        } else {
            nav.get(bmc.as_ref()).await.map_err(Error::Bmc)
        }
        .map(|data| Self {
            bmc: bmc.clone(),
            data,
            config: config.clone(),
        })
    }

    /// Create from existing data.
    pub(crate) fn from_data(bmc: NvBmc<B>, data: ManagerAccount, config: Config) -> Self {
        Self {
            bmc,
            data: Arc::new(data),
            config,
        }
    }

    /// Raw `ManagerAccount` data.
    #[must_use]
    pub fn raw(&self) -> Arc<ManagerAccount> {
        self.data.clone()
    }

    /// Account is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.data.enabled.is_none_or(identity)
    }

    /// Update the account.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns an error if the server responds with an error or if the
    /// response cannot be parsed.
    pub async fn update(&self, update: &ManagerAccountUpdate) -> Result<Self, Error<B>> {
        let account = self.update_with_patch(update).await?;
        Ok(Self {
            config: self.config.clone(),
            bmc: self.bmc.clone(),
            data: Arc::new(account),
        })
    }

    /// Update the account's password.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns an error if the server responds with an error or if the
    /// response cannot be parsed.
    pub async fn update_password(&self, password: String) -> Result<Self, Error<B>> {
        self.update(
            &ManagerAccountUpdate::builder()
                .with_password(password)
                .build(),
        )
        .await
    }

    /// Update the account's user name.
    ///
    /// Returns the new (updated) account.
    ///
    /// # Errors
    ///
    /// Returns an error if the server responds with an error or if the
    /// response cannot be parsed.
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
    /// Returns an error if deletion fails.
    pub async fn delete(&self) -> Result<(), Error<B>> {
        if self.config.disable_account_on_delete {
            self.update(&ManagerAccountUpdate::builder().with_enabled(false).build())
                .await
                .map(|_| ())
        } else {
            self.data
                .delete(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)
                .map(|_| ())
        }
    }
}
