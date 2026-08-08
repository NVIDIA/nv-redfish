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

use crate::schema::service_root::ServiceRoot;

#[cfg(feature = "accounts")]
use crate::account::SlotDefinedConfig as SlotDefinedUserAccountsConfig;

/// Object that provides quirks of individual platforms. On first root
/// retrieval we classify platform and then apply specific workarounds
/// for each individual platform class.
pub struct BmcQuirks {
    platform: Option<Platform>,
    /// Vendor is NVIDIA regardless of which platform class the
    /// product was classified into. Quirks that apply to the whole
    /// vendor family gate on this instead of enumerating platforms,
    /// so a new product-specific match arm cannot silently drop them.
    #[cfg(feature = "event-service")]
    nvidia_vendor: bool,
}

// Platform shouldn't be considered as vendor. Actually it is class of
// devices that have the same set of quirks.
#[derive(PartialEq, Eq)]
enum Platform {
    Hpe,
    Dell,
    AmiViking,
    AmiGb300,
    VeraRubin,
    NvidiaDpu,
    Anonymous1_9_0,
}

impl BmcQuirks {
    pub fn new(root: &ServiceRoot) -> Self {
        let vendor_str = root.vendor.as_ref().and_then(Option::as_deref);
        let redfish_version_str = root.redfish_version.as_deref();
        let product_str = root.product.as_ref().and_then(Option::as_deref);
        // The GB300 host BMC exposes an AMI OEM `RtpVersion` in the service
        // root; use it to distinguish GB300 from other AMI BMCs so the
        // expand workaround is not applied to every AMI platform.
        let rtp_version = root
            .base
            .base
            .oem
            .as_ref()
            .and_then(|oem| oem.additional_properties.get("Ami"))
            .and_then(|ami| ami.get("RtpVersion"))
            .and_then(|v| v.as_str());
        let platform = match vendor_str {
            Some("HPE") => Some(Platform::Hpe),
            Some("Dell") => Some(Platform::Dell),
            Some("AMI") if redfish_version_str == Some("1.11.0") => Some(Platform::AmiViking),
            Some("AMI") if rtp_version == Some("13.09.1") => Some(Platform::AmiGb300),
            Some("NVIDIA") if product_str == Some("VR NVL72") => Some(Platform::VeraRubin),
            // Other NVIDIA host BMC products have no platform-specific
            // quirks; NVIDIA-family quirks gate on `nvidia_vendor`.
            // BF3 service roots use this product name with an `Nvidia` vendor.
            Some("Nvidia") if matches!(product_str, Some("Nvidia-BMCMezz" | "BlueField-3 DPU")) => {
                Some(Platform::NvidiaDpu)
            }
            None if redfish_version_str == Some("1.9.0") => Some(Platform::Anonymous1_9_0),
            _ => None,
        };
        Self {
            platform,
            // Both vendor string casings are in the wild: "NVIDIA" on
            // host BMCs and "Nvidia" on BlueField DPUs.
            #[cfg(feature = "event-service")]
            nvidia_vendor: matches!(vendor_str, Some("NVIDIA" | "Nvidia")),
        }
    }

    // Account type is required according to schema specification
    // (marked with Redfish.Required annotation) but some vendors
    // ignores this flag. A workaround for this bug is supported by
    // `nv-redfish`.
    #[cfg(feature = "accounts")]
    pub(crate) fn bug_no_account_type_in_accounts(&self) -> bool {
        self.platform == Some(Platform::Hpe)
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
    pub(crate) const fn bug_missing_root_nav_properties(&self) -> bool {
        match self.platform {
            // 1. There are situations when Viking doesn't provide any
            //    navigation properties in root before BMC reset.
            // 2. LiteonPowershelf doesn't provide Systems
            Some(Platform::AmiViking | Platform::Anonymous1_9_0) => true,
            _ => false,
        }
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

    /// NVIDIA DPU sometimes returns empty string UUID in
    /// chassis/computer system payloads when DPU is in NIC mode.
    #[cfg(any(feature = "chassis", feature = "computer-systems"))]
    pub(crate) fn bug_empty_uuid_field(&self) -> bool {
        self.platform == Some(Platform::NvidiaDpu)
    }

    /// NVIDIA DPU serves the computer system `Oem.Nvidia` object as a
    /// separate resource, inlining only a partially expanded stub, and
    /// puts `BaseMAC` and `Mode` in it -- neither of which the NVIDIA
    /// OEM CSDL declares. Both the extra fetch and reading the two
    /// properties out of the raw body are restricted to this platform.
    #[cfg(all(feature = "computer-systems", feature = "oem-nvidia"))]
    pub(crate) fn bug_dpu_oem_computer_system(&self) -> bool {
        self.platform == Some(Platform::NvidiaDpu)
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
    ///
    /// Gated on the NVIDIA vendor family rather than platform classes:
    /// product-specific match arms split off the generic NVIDIA
    /// platform twice (VeraRubin, NvSwitch) and silently dropped this
    /// patch. The patch is a no-op on firmware that provides
    /// `MemberId`.
    #[cfg(feature = "event-service")]
    pub(crate) const fn event_service_sse_no_member_id(&self) -> bool {
        self.nvidia_vendor
    }

    /// In some implementations, Event records in SSE payload use compact
    /// timezone offsets in `EventTimestamp` (for example, `-0600`).
    #[cfg(feature = "event-service")]
    pub(crate) fn event_service_sse_wrong_timestamp_offset(&self) -> bool {
        self.platform == Some(Platform::Dell)
    }

    /// SSE payload does not include `@odata.id`.
    #[cfg(feature = "event-service")]
    #[allow(clippy::unused_self)]
    pub(crate) const fn event_service_sse_no_odata_id(&self) -> bool {
        true
    }

    /// Vera Rubin host BMCs report composite `BootOrder` entries such as
    /// `"Boot0019: Ubuntu"` while boot option resources use the bare reference.
    #[cfg(feature = "computer-systems")]
    pub(crate) fn vera_rubin_composite_boot_order_entries(&self) -> bool {
        self.platform == Some(Platform::VeraRubin)
    }

    /// Vikings provide wrong elements in computer system
    /// collection. This function returns ODataId filter function for
    /// these collections.
    #[cfg(feature = "computer-systems")]
    pub(crate) fn filter_computer_system_odata_ids(&self) -> Option<fn(&str) -> bool> {
        (self.platform == Some(Platform::AmiViking)).then_some(|odata_id| {
            odata_id.ends_with("/DGX") || odata_id.ends_with("/HGX_Baseboard_0")
        })
    }

    /// Vikings provide wrong elements in manager
    /// collection. This function returns ODataId filter function for
    /// these collections.
    #[cfg(feature = "managers")]
    pub(crate) fn filter_manager_odata_ids(&self) -> Option<fn(&str) -> bool> {
        (self.platform == Some(Platform::AmiViking)).then_some(|odata_id| {
            odata_id.ends_with("/BMC")
                || odata_id.ends_with("/HGX_BMC_0")
                || odata_id.ends_with("/HGX_FabricManager_0")
        })
    }

    /// In some cases we expand is not working according to spec,
    /// if it is the case for specific chassis, we would disable
    /// expand api.
    ///
    /// AMI host BMCs (Viking and the GB300-class `Ami`) return `$expand`
    /// responses that drop Required fields (Id/Name/ChassisType) from embedded
    /// members; the standalone resource GETs are complete, so disabling expand
    /// makes nv-redfish fetch each member individually and parse correctly.
    pub(crate) const fn expand_is_not_working_properly(&self) -> bool {
        matches!(
            self.platform,
            Some(Platform::AmiViking | Platform::AmiGb300)
        )
    }
}

#[cfg(test)]
#[cfg(feature = "event-service")]
mod tests {
    use super::BmcQuirks;
    use crate::schema::service_root::ServiceRoot;

    fn quirks(vendor: &str, product: &str) -> BmcQuirks {
        let root: ServiceRoot = serde_json::from_value(serde_json::json!({
            "@odata.id": "/redfish/v1",
            "@odata.type": "#ServiceRoot.v1_9_0.ServiceRoot",
            "Id": "RootService",
            "Name": "Root Service",
            "RedfishVersion": "1.9.0",
            "Vendor": vendor,
            "Product": product,
            "Links": {
                "Sessions": { "@odata.id": "/redfish/v1/SessionService/Sessions" }
            },
        }))
        .expect("service root");
        BmcQuirks::new(&root)
    }

    // The SSE MemberId patch must stay applied to the whole NVIDIA
    // vendor family: platform match arms that split a product off the
    // generic NVIDIA arm have silently dropped it twice (VeraRubin,
    // NvSwitch). Gating on the vendor makes future product splits
    // unable to recreate that bug.
    #[test]
    fn sse_member_id_patch_covers_nvidia_vendor_family() {
        assert!(quirks("NVIDIA", "Generic").event_service_sse_no_member_id());
        assert!(quirks("NVIDIA", "VR NVL72").event_service_sse_no_member_id());
        assert!(quirks("NVIDIA", "P3809").event_service_sse_no_member_id());
        assert!(quirks("Nvidia", "BlueField-3 DPU").event_service_sse_no_member_id());
        assert!(quirks("Nvidia", "FutureProduct").event_service_sse_no_member_id());
        assert!(!quirks("Dell", "PowerEdge").event_service_sse_no_member_id());
    }
}
