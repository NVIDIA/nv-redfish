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
//! The resource is the NVIDIA `NvidiaManager` OEM object and is returned
//! as the compiled schema type.
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

use crate::oem::nvidia::schema::nvidia_manager::v1_9_0::NvidiaManager as NvidiaManagerSchema;
use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::patch_support::JsonValue;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataId;
use serde::Serialize;
use std::sync::Arc;

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

/// Represents a NVIDIA extension of manager in the BMC.
pub struct NvidiaManager<B: Bmc> {
    data: Arc<NvidiaManagerSchema>,
    /// Resource linked from `Oem.Nvidia`. `None` on every platform other
    /// than the BlueField DPU; see the module documentation.
    dpu_resource: Option<ODataId>,
    bmc: NvBmc<B>,
}

impl<B: Bmc> NvidiaManager<B> {
    /// Create a new NVIDIA manager handle.
    ///
    /// Returns `Ok(None)` when the OEM payload carries no NVIDIA object.
    pub(crate) fn new(bmc: &NvBmc<B>, oem: &ResourceOemSchema) -> Result<Option<Self>, Error<B>> {
        let Some(nvidia) = oem_value(oem, OEM_KEY) else {
            return Ok(None);
        };
        let data = serde_json::from_value(nvidia.clone()).map_err(Error::Json)?;
        let dpu_resource = bmc
            .quirks
            .bug_dpu_oem_manager()
            .then(|| nvidia.get("@odata.id").and_then(JsonValue::as_str))
            .flatten()
            .map(|id| ODataId::from(id.to_owned()));
        Ok(Some(Self {
            data: Arc::new(data),
            dpu_resource,
            bmc: bmc.clone(),
        }))
    }

    /// Get the raw schema data for this NVIDIA manager.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<NvidiaManagerSchema> {
        self.data.clone()
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
