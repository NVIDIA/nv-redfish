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

//! Support NVIDIA Bluefield ComputerSystem OEM extension.

use crate::oem::nvidia::bluefield::schema::redfish::nvidia_computer_system::NvidiaComputerSystem as NvidiaComputerSystemSchema;
use crate::schema::redfish::resource::Oem as ResourceOemSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use serde::Deserialize;
use std::marker::PhantomData;
use std::sync::Arc;
use tagged_types::TaggedType;

#[derive(Deserialize)]
struct Oem {
    #[serde(rename = "Nvidia")]
    nvidia: Option<NavProperty<NvidiaComputerSystemSchema>>,
}

#[doc(inline)]
pub use crate::oem::nvidia::bluefield::schema::redfish::nvidia_computer_system::Mode;

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
    _marker: PhantomData<B>,
}

impl<B: Bmc> NvidiaComputerSystem<B> {
    /// Create a new computer system handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        oem: &ResourceOemSchema,
    ) -> Result<Option<Self>, Error<B>> {
        let oem: Oem =
            serde_json::from_value(oem.additional_properties.clone()).map_err(Error::Json)?;
        if let Some(p) = oem.nvidia {
            p.get(bmc.as_ref()).await.map_err(Error::Bmc).map(|data| {
                Some(Self {
                    data,
                    _marker: PhantomData,
                })
            })
        } else {
            Ok(None)
        }
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
    #[must_use]
    pub fn base_mac(&self) -> Option<BaseMac<&str>> {
        self.data.base_mac.as_deref().map(BaseMac::new)
    }

    /// Get mode of the Bluefield device.
    ///
    /// Getting mode from directly from OEM extension is supported
    /// only by Bluefield 3.
    #[must_use]
    pub fn mode(&self) -> Option<Mode> {
        self.data.mode
    }
}
