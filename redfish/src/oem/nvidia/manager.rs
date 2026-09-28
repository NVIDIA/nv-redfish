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

//! Support NVIDIA Manager OEM extension.
//!
//! The BlueField DPU links a separate resource from the manager's
//! `Oem.Nvidia` object and keeps the BMC rshim state in it:
//!
//! ```text
//! GET /redfish/v1/Managers/Bluefield_BMC/Oem/Nvidia
//! {"BmcRShim": {"BmcRShimEnabled": true}}
//! ```
//!
//! The NVIDIA OEM CSDL does not declare `BmcRShim`, so it is patched as
//! a platform quirk, only on the platform detected as the DPU.
//! BlueField-4 does not publish the resource.

use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::patch_support::JsonValue;
use crate::schema::manager::Manager as ManagerSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataId;
use serde::Serialize;

#[derive(Serialize)]
struct BmcRshimUpdate {
    #[serde(rename = "BmcRShim")]
    bmc_rshim: BmcRshimState,
}

#[derive(Serialize)]
struct BmcRshimState {
    #[serde(rename = "BmcRShimEnabled")]
    enabled: bool,
}

/// NVIDIA OEM extension of a manager.
pub struct NvidiaManager<B: Bmc> {
    bmc: NvBmc<B>,
    /// Resource linked from `Oem.Nvidia`. `None` on every platform other
    /// than the BlueField DPU; see the module documentation.
    dpu_resource: Option<ODataId>,
}

impl<B: Bmc> NvidiaManager<B> {
    /// Create a new NVIDIA manager handle.
    ///
    /// Returns `None` when the manager carries no `Oem.Nvidia` object.
    pub(crate) fn new(bmc: &NvBmc<B>, manager: &ManagerSchema) -> Option<Self> {
        let nvidia = oem_value(manager.oem.as_ref()?, OEM_KEY)?;
        let dpu_resource = bmc
            .quirks
            .bug_dpu_oem_manager()
            .then(|| nvidia.get("@odata.id").and_then(JsonValue::as_str))
            .flatten()
            .map(|id| ODataId::from(id.to_owned()));
        Some(Self {
            bmc: bmc.clone(),
            dpu_resource,
        })
    }

    /// Enable or disable the BMC side of the DPU rshim interface.
    ///
    /// Patches `BmcRShim.BmcRShimEnabled` on the resource linked from
    /// `Oem.Nvidia`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ActionNotAvailable`] on any platform other than
    /// the BlueField DPU or when the DPU links no such resource, or an
    /// error if the update fails.
    pub async fn set_bmc_rshim_enabled(
        &self,
        enabled: bool,
    ) -> Result<ModificationResponse<()>, Error<B>> {
        let id = self
            .dpu_resource
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;
        let update = BmcRshimUpdate {
            bmc_rshim: BmcRshimState { enabled },
        };
        self.bmc
            .as_ref()
            .update::<_, JsonValue>(id, None, &update)
            .await
            .map(|response| response.map_entity(|_| ()))
            .map_err(Error::Bmc)
    }
}
