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
//! * that body carries `BaseMAC` and `Mode`. Neither is declared by the
//!   CSDL the device publishes, and the NVIDIA OEM schema set not only
//!   omits them but marks the type `OData.AdditionalProperties=false`,
//!   so no schema route to them exists or should be invented.
//!
//! Both apply only on the platform detected as the DPU. Drop
//! [`NvidiaComputerSystem::base_mac`] and [`NvidiaComputerSystem::mode`]
//! once firmware either stops sending them or declares them properly.

use crate::oem::nvidia::schema::nvidia_computer_system::NvidiaComputerSystem as NvidiaComputerSystemSchema;
use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::ODataId;
use std::marker::PhantomData;
use std::sync::Arc;
use tagged_types::TaggedType;

/// Operating mode of a BlueField device.
///
/// Undeclared by the NVIDIA OEM schema; see the module documentation.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// This BlueField device works as a regular NIC for the host.
    NicMode,
    /// This BlueField device is a 'bump in a wire' that controls packet
    /// processing.
    DpuMode,
    /// Fallback for modes this version of the library does not know.
    UnsupportedValue,
}

impl Mode {
    fn parse(v: &str) -> Self {
        match v {
            "NicMode" => Self::NicMode,
            "DpuMode" => Self::DpuMode,
            _ => Self::UnsupportedValue,
        }
    }
}

/// Base MAC address of the Bluefield DPU as reported by the device.
pub type BaseMac<T> = TaggedType<T, BaseMacTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum BaseMacTag {}

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
    _marker: PhantomData<B>,
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
                _marker: PhantomData,
            }));
        }
        let data = serde_json::from_value(nvidia.clone()).map_err(Error::Json)?;
        Ok(Some(Self {
            data: Arc::new(data),
            dpu_body: None,
            _marker: PhantomData,
        }))
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
}
