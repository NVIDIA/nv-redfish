// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use crate::resource::oem_id_from_resource;
use crate::schema::redfish::service_root::ServiceRoot;

#[cfg(feature = "accounts")]
use crate::account::SlotDefinedConfig as SlotDefinedUserAccountsConfig;

/// Object that provides quirks of individual platforms. On first root
/// retrieval we classify platform and then apply specific workarounds
/// for each individual platform class.
pub struct BmcQuirks {
    platform: Option<Platform>,
}

// Platform shouldn't be considered as vendor. Actually it is class of
// devices that have the same set of quirks.
#[derive(PartialEq, Eq)]
enum Platform {
    Hpe,
    Dell,
    AmiViking,
    Nvidia,
    LenovoAmi,
}

impl BmcQuirks {
    pub fn new(root: &ServiceRoot) -> Self {
        let vendor_str = root.vendor.as_ref().and_then(Option::as_deref);
        let oem_id_str = oem_id_from_resource(&root.base);
        let redfish_version_str = root.redfish_version.as_deref();
        let platform = match vendor_str {
            Some("HPE") => Some(Platform::Hpe),
            Some("Dell") => Some(Platform::Dell),
            Some("Lenovo") if oem_id_str == Some("Ami") => Some(Platform::LenovoAmi),
            Some("AMI") if redfish_version_str == Some("1.11.0") => Some(Platform::AmiViking),
            Some("NVIDIA") => Some(Platform::Nvidia),
            _ => None,
        };
        Self { platform }
    }

    // Account type is required according to schema specification
    // (marked with Redfish.Required annotation) but some vendors
    // ignores this flag. A workaround for this bug is supported by
    // `nv-redfish`.
    #[cfg(feature = "accounts")]
    pub(crate) fn bug_no_account_type_in_accounts(&self) -> bool {
        self.platform == Some(Platform::Hpe)
    }

    #[cfg(feature = "accounts")]
    pub(crate) fn bug_null_in_remote_role_mapping(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    // In some implementations BMC cannot create / delete Redfish
    // accounts but have pre-created accounts (slots). Workflow is as
    // following: to "create" new account user should update
    // precreated account with new parameters and enable it. To delete
    // account user should just disable it.
    #[cfg(feature = "accounts")]
    pub(crate) fn slot_defined_user_accounts(&self) -> Option<SlotDefinedUserAccountsConfig> {
        self.platform.as_ref().and_then(|platform| {
            (platform == &Platform::Dell).then_some(SlotDefinedUserAccountsConfig {
                min_slot: Some(3),
                hide_disabled: true,
                disable_account_on_delete: true,
            })
        })
    }

    // In some implementations BMC ReleaseDate is incorrectly set to
    // 00:00:00Z in FirmwareInventory (which is
    // SoftwareInventoryCollection).
    #[cfg(feature = "update-service")]
    pub(crate) fn fw_inventory_wrong_release_date(&self) -> bool {
        self.platform == Some(Platform::Dell)
    }

    /// In some cases there is addtional fields in Links.ContainedBy in
    /// Chassis resource, this flag aims to patch this invalid links
    #[cfg(feature = "chassis")]
    pub(crate) fn bug_invalid_contained_by_fields(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    /// Missing navigation properties in root object.
    #[cfg(any(
        feature = "chassis",
        feature = "computer-systems",
        feature = "managers",
        feature = "update-service",
    ))]
    pub(crate) fn bug_missing_root_nav_properties(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    /// Missing chassis type property in Chassis resource. This
    /// property is Required in according to specification but some
    /// systems doesn't provide it.
    #[cfg(feature = "chassis")]
    pub(crate) fn bug_missing_chassis_type_field(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    /// Missing Name property in Chassis resource. This property is
    /// required in any resource.
    #[cfg(feature = "chassis")]
    pub(crate) fn bug_missing_chassis_name_field(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    /// Missing Name property in Chassis resource. This property is
    /// required in any resource.
    #[cfg(feature = "update-service")]
    pub(crate) fn bug_missing_update_service_name_field(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }

    /// In some implementations BMC ReleaseDate is incorrectly set to
    /// "0000-00-00T00:00:00+00:00" in ComputerSystem/LastResetTime
    /// This prevents ComputerSystem to be correctly parsed because
    /// this is invalid Edm.DateTimeOffset.
    #[cfg(feature = "computer-systems")]
    pub(crate) fn computer_systems_wrong_last_reset_time(&self) -> bool {
        self.platform == Some(Platform::Dell)
    }

    /// In some implementations, Event records in SSE payload do not include
    /// `MemberId`.
    #[cfg(feature = "event-service")]
    pub(crate) fn event_service_sse_no_member_id(&self) -> bool {
        self.platform == Some(Platform::Nvidia)
    }

    /// In some implementations, Event records in SSE payload use compact
    /// timezone offsets in `EventTimestamp` (for example, `-0600`).
    #[cfg(feature = "event-service")]
    pub(crate) fn event_service_sse_wrong_timestamp_offset(&self) -> bool {
        self.platform == Some(Platform::Dell)
    }

    /// In some implementations, Event records in SSE payload use unsupported
    /// values in `EventType`.
    #[cfg(feature = "event-service")]
    pub(crate) fn event_service_sse_wrong_event_type(&self) -> bool {
        self.platform == Some(Platform::Nvidia)
    }

    /// SSE payload does not include `@odata.id`.
    #[cfg(feature = "event-service")]
    #[allow(clippy::unused_self)]
    pub(crate) const fn event_service_sse_no_odata_id(&self) -> bool {
        true
    }

    /// In some cases we expand is not working according to spec,
    /// if it is the case for specific chassis, we would disable
    /// expand api
    pub(crate) fn expand_is_not_working_properly(&self) -> bool {
        self.platform == Some(Platform::AmiViking)
    }
}
