// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Support NVIDIA ComputerSystem OEM extension.
//!
//! The resource is the NVIDIA `NvidiaComputerSystem` OEM object and is
//! returned as the compiled schema type.
//!
//! The BlueField DPU diverges from that schema twice, and both
//! divergences are handled as platform quirks rather than as a schema
//! of their own:
//!
//! * it serves the object as a separate resource and inlines only a
//!   partially expanded stub under `Oem.Nvidia`, so the body has to be
//!   fetched from `@odata.id`;
//! * that body carries `BaseMAC`, `Mode`, `HostRshim` and the
//!   `#Mode.Set`, `#HostRshim.Set` and `#SOC.ForceReset` actions. None
//!   of them is declared by the CSDL the device publishes, and the
//!   NVIDIA OEM schema set not only omits them but marks the type
//!   `OData.AdditionalProperties=false`, so no schema route to them
//!   exists or should be invented. The actions are typed here instead,
//!   with their targets taken from the body.
//!
//! Both apply only on the platform detected as the DPU. BlueField-4
//! serves the object without any of these; its DPU mode and host
//! privileges live on the network adapter instead. BlueField-3 with BMC
//! firmware 26.04 or later also links a host privilege configuration
//! from its network adapters, alongside these. Drop the quirk accessors
//! once firmware either stops sending them or declares them properly.

use crate::oem::nvidia::schema::nvidia_computer_system::NvidiaComputerSystem as NvidiaComputerSystemSchema;
use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Action;
use nv_redfish_core::ActionError;
use nv_redfish_core::Bmc;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

/// Operating mode of a BlueField device.
///
/// Undeclared by the NVIDIA OEM schema; see the module documentation.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    /// This BlueField device works as a regular NIC for the host.
    NicMode,
    /// This BlueField device is a 'bump in a wire' that controls packet
    /// processing.
    DpuMode,
    /// Fallback for modes this version of the library does not know.
    #[serde(other)]
    UnsupportedValue,
}

impl Mode {
    /// Map a reported mode, keeping unknown ones as `UnsupportedValue`.
    fn parse(v: &str) -> Self {
        match v {
            "NicMode" => Self::NicMode,
            "DpuMode" => Self::DpuMode,
            _ => Self::UnsupportedValue,
        }
    }
}

/// State of the host-side rshim interface of a BlueField device.
///
/// Undeclared by the NVIDIA OEM schema; see the module documentation.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HostRshim {
    /// The host can reach the DPU over rshim.
    Enabled,
    /// The host cannot reach the DPU over rshim.
    Disabled,
    /// Fallback for states this version of the library does not know.
    UnsupportedValue,
}

impl HostRshim {
    /// Map a reported state, keeping unknown ones as `UnsupportedValue`.
    fn parse(v: &str) -> Self {
        match v {
            "Enabled" => Self::Enabled,
            "Disabled" => Self::Disabled,
            _ => Self::UnsupportedValue,
        }
    }
}

#[derive(Serialize)]
struct ModeSetParams {
    #[serde(rename = "Mode")]
    mode: Mode,
}

#[derive(Serialize)]
struct HostRshimSetParams {
    #[serde(rename = "HostRshim")]
    host_rshim: &'static str,
}

#[derive(Serialize)]
struct NoParams {}

pub use crate::oem::nvidia::BaseMac;
#[doc(hidden)]
pub use crate::oem::nvidia::BaseMacTag;

/// Represents a NVIDIA extension of computer system in the BMC.
///
/// Provides access to system information and sub-resources such as processors.
pub struct NvidiaComputerSystem<B: Bmc> {
    data: Arc<NvidiaComputerSystemSchema>,
    /// Response body kept verbatim so the undeclared DPU properties
    /// stay reachable: the schema type cannot carry them. `None` on
    /// every other platform, which is what keeps those properties from
    /// being reported where they are not expected.
    dpu_body: Option<Arc<JsonValue>>,
    bmc: NvBmc<B>,
}

impl<B: Bmc> NvidiaComputerSystem<B> {
    /// Create a new computer system handle.
    ///
    /// Returns `Ok(None)` when the OEM payload carries no NVIDIA object.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        oem: &ResourceOemSchema,
    ) -> Result<Option<Self>, Error<B>> {
        let Some(nvidia) = oem_value(oem, OEM_KEY) else {
            return Ok(None);
        };
        if bmc.quirks.bug_dpu_oem_computer_system() {
            // The inlined object is only a partially expanded stub, so
            // the body at `@odata.id` is the sole reliable source.
            let Some(id) = nvidia.get("@odata.id").and_then(JsonValue::as_str) else {
                return Ok(None);
            };
            let body = Payload::get_raw(bmc.as_ref(), &ODataId::from(id.to_owned())).await?;
            let data = serde_json::from_value(body.clone()).map_err(Error::Json)?;
            return Ok(Some(Self {
                data: Arc::new(data),
                dpu_body: Some(Arc::new(body)),
                bmc: bmc.clone(),
            }));
        }
        let data = serde_json::from_value(nvidia.clone()).map_err(Error::Json)?;
        Ok(Some(Self {
            data: Arc::new(data),
            dpu_body: None,
            bmc: bmc.clone(),
        }))
    }

    /// Parse the advertised `Oem.Nvidia` action `name`.
    ///
    /// The actions are undeclared, so each is parsed only when invoked; a
    /// malformed action fails that call instead of the whole extension.
    fn dpu_action<T>(&self, name: &str) -> Result<Action<T, ()>, Error<B>> {
        let action = self
            .dpu_body
            .as_ref()
            .and_then(|body| body.get("Actions"))
            .and_then(|actions| actions.get(name))
            .filter(|action| !action.is_null())
            .ok_or(Error::ActionNotAvailable)?;
        Action::deserialize(action).map_err(Error::Json)
    }

    /// Get the raw schema data for this NVIDIA computer system.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<NvidiaComputerSystemSchema> {
        self.data.clone()
    }

    /// Get base MAC address of the device.
    ///
    /// Quirk: undeclared by the schema, read from the response body.
    /// `None` on any platform other than the BlueField DPU; see the
    /// module documentation.
    #[must_use]
    pub fn base_mac(&self) -> Option<BaseMac<&str>> {
        self.dpu_body
            .as_ref()?
            .get("BaseMAC")
            .and_then(JsonValue::as_str)
            .map(BaseMac::new)
    }

    /// Get mode of the Bluefield device.
    ///
    /// Quirk: undeclared by the schema, read from the response body.
    /// `None` on any platform other than the BlueField DPU; see the
    /// module documentation. Reporting the mode through the OEM
    /// extension directly is supported only by Bluefield 3.
    #[must_use]
    pub fn mode(&self) -> Option<Mode> {
        self.dpu_body
            .as_ref()?
            .get("Mode")
            .and_then(JsonValue::as_str)
            .map(Mode::parse)
    }

    /// Get the state of the host-side rshim interface.
    ///
    /// Quirk: undeclared by the schema, read from the response body.
    /// `None` on any platform other than the BlueField DPU and on DPUs
    /// that do not report it, such as BlueField-4.
    #[must_use]
    pub fn host_rshim(&self) -> Option<HostRshim> {
        self.dpu_body
            .as_ref()?
            .get("HostRshim")
            .and_then(JsonValue::as_str)
            .map(HostRshim::parse)
    }

    /// Switch the BlueField device between NIC and DPU mode.
    ///
    /// Invokes the `#Mode.Set` action advertised in the DPU's
    /// `Oem.Nvidia` body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ActionNotAvailable`] when the DPU does not
    /// advertise `#Mode.Set` (for example BlueField-4, which sets the
    /// mode on its network adapter), or an error if invoking the action
    /// fails.
    pub async fn set_mode(&self, mode: Mode) -> Result<ModificationResponse<()>, Error<B>>
    where
        B::Error: ActionError,
    {
        let action = self.dpu_action("#Mode.Set")?;
        action
            .run(self.bmc.as_ref(), &ModeSetParams { mode })
            .await
            .map_err(Error::Bmc)
    }

    /// Enable or disable the host-side rshim interface.
    ///
    /// Invokes the `#HostRshim.Set` action advertised in the DPU's
    /// `Oem.Nvidia` body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ActionNotAvailable`] when the DPU does not
    /// advertise `#HostRshim.Set`, or an error if invoking the action
    /// fails.
    pub async fn set_host_rshim(&self, enabled: bool) -> Result<ModificationResponse<()>, Error<B>>
    where
        B::Error: ActionError,
    {
        let action = self.dpu_action("#HostRshim.Set")?;
        let host_rshim = if enabled { "Enabled" } else { "Disabled" };
        action
            .run(self.bmc.as_ref(), &HostRshimSetParams { host_rshim })
            .await
            .map_err(Error::Bmc)
    }

    /// Force a reset of the DPU's Arm SoC.
    ///
    /// Invokes the `#SOC.ForceReset` action advertised in the DPU's
    /// `Oem.Nvidia` body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ActionNotAvailable`] when the DPU does not
    /// advertise `#SOC.ForceReset`, or an error if invoking the action
    /// fails.
    pub async fn soc_force_reset(&self) -> Result<ModificationResponse<()>, Error<B>>
    where
        B::Error: ActionError,
    {
        let action = self.dpu_action("#SOC.ForceReset")?;
        action
            .run(self.bmc.as_ref(), &NoParams {})
            .await
            .map_err(Error::Bmc)
    }
}
