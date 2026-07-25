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
//! Boot options
//!

use crate::bmc_quirks::BmcQuirks;
use crate::computer_system::BootOptionReference;
use crate::schema::boot_option::BootOption as BootOptionSchema;
use crate::schema::boot_option_collection::BootOptionCollection as BootOptionCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::convert::identity;
use std::marker::PhantomData;
use std::sync::Arc;
use tagged_types::TaggedType;

/// Boot options collection.
///
/// Provides functions to access collection members.
pub struct BootOptionCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<BootOptionCollectionSchema>,
}

impl<B: Bmc> BootOptionCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<BootOptionCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = bmc.expand_property(nav).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all managers available in this BMC.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching manager data fails.
    pub async fn members(&self) -> Result<Vec<BootOption<B>>, Error<B>> {
        let mut members = Vec::new();
        for m in &self.collection.members {
            members.push(BootOption::new(&self.bmc, m).await?);
        }
        Ok(members)
    }
}

/// The UEFI device path to access this UEFI boot option.
///
/// Nv-redfish keeps open underlying type for `UefiDevicePath` because it
/// can really be represented by any implementation of UEFI's device path.
pub type UefiDevicePath<T> = TaggedType<T, UefiDevicePathTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum UefiDevicePathTag {}

/// The user-readable display name of the boot option that appears in
/// the boot order list in the user interface.
pub type DisplayName<T> = TaggedType<T, DisplayNameTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum DisplayNameTag {}

/// Boot option.
///
/// Provides functions to access boot option.
pub struct BootOption<B: Bmc> {
    data: Arc<BootOptionSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> BootOption<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<BootOptionSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this boot option.
    #[must_use]
    pub fn raw(&self) -> Arc<BootOptionSchema> {
        self.data.clone()
    }

    ///
    /// Boot option reference.
    #[must_use]
    pub fn boot_reference(&self) -> BootOptionReference<&str> {
        self.data.boot_option_reference.as_deref().map_or_else(
            || BootOptionReference::new(self.id().inner()),
            BootOptionReference::new,
        )
    }

    /// Returns whether this boot option is referenced by a `BootOrder` entry.
    ///
    /// Vera Rubin firmware may report composite boot-order strings such as
    /// `"Boot0019: Ubuntu"`; when `quirks` identifies that platform, the
    /// display-name suffix is stripped before comparing to
    /// [`Self::boot_reference`]. All other platforms use exact reference match.
    #[must_use]
    pub(crate) fn matches_boot_order_entry(
        &self,
        entry: BootOptionReference<&str>,
        quirks: &BmcQuirks,
    ) -> bool {
        matches_boot_order_entry(self, entry, quirks)
    }

    /// An indication of whether the boot option is enabled.
    #[must_use]
    pub fn enabled(&self) -> Option<bool> {
        self.data.boot_option_enabled.and_then(identity)
    }

    /// The user-readable display name of the boot option that appears
    /// in the boot order list in the user interface.
    #[must_use]
    pub fn display_name(&self) -> Option<DisplayName<&str>> {
        self.data
            .display_name
            .as_ref()
            .and_then(Option::as_ref)
            .map(String::as_str)
            .map(DisplayName::new)
    }

    /// The UEFI device path to access this UEFI boot option.
    #[must_use]
    pub fn uefi_device_path(&self) -> Option<UefiDevicePath<&str>> {
        self.data
            .uefi_device_path
            .as_ref()
            .and_then(Option::as_ref)
            .map(String::as_str)
            .map(UefiDevicePath::new)
    }
}

impl<B: Bmc> Resource for BootOption<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}

/// Returns whether `option` is referenced by a `BootOrder` entry on this BMC.
#[must_use]
pub(crate) fn matches_boot_order_entry<B: Bmc>(
    option: &BootOption<B>,
    entry: BootOptionReference<&str>,
    quirks: &BmcQuirks,
) -> bool {
    let entry_reference = if quirks.vera_rubin_composite_boot_order_entries() {
        vera_rubin_boot_order_entry_reference(entry.inner())
    } else {
        entry.inner()
    };
    entry_reference == *option.boot_reference().inner()
}

/// Strips the Vera Rubin `"<reference>: <display name>"` boot-order suffix.
fn vera_rubin_boot_order_entry_reference(entry: &str) -> &str {
    entry
        .split_once(": ")
        .map_or(entry, |(reference, _)| reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vera_rubin_boot_order_entry_reference_strips_display_name_suffix() {
        assert_eq!(
            vera_rubin_boot_order_entry_reference("Boot0019: Ubuntu"),
            "Boot0019"
        );
        assert_eq!(
            vera_rubin_boot_order_entry_reference("Boot0010: UEFI HTTPv4 (MAC:AA)"),
            "Boot0010"
        );
        assert_eq!(
            vera_rubin_boot_order_entry_reference("Boot0010"),
            "Boot0010"
        );
    }
}
