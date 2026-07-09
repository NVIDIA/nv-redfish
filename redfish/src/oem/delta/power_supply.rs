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

//! Support Delta Energy Systems `PowerSupply` OEM extension.

use crate::oem::delta::schema::delta_energy_systems_power_supply::PowerSupply as DeltaPowerSupplySchema;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use nv_redfish_core::Bmc;
use serde::Deserialize;
use std::convert::identity;
use std::marker::PhantomData;
use std::sync::Arc;

/// Vendor key under which Delta nests its OEM `PowerSupply` extension.
pub const OEM_KEY: &str = "deltaenergysystems";

/// Delta Energy Systems OEM extension for a `PowerSupply`.
///
/// Delta power shelves do not populate the standard Redfish `PowerState`
/// field on their power supplies; the authoritative indication of whether a
/// PSU is outputting power is carried in this OEM object under
/// `Oem/deltaenergysystems`.
pub struct DeltaPowerSupply<B: Bmc> {
    data: Arc<DeltaPowerSupplySchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> DeltaPowerSupply<B> {
    /// Create a Delta OEM power supply handle from a power supply's `Oem` bag.
    ///
    /// Returns `Ok(None)` when the OEM payload does not contain Delta power
    /// supply data.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing the Delta OEM data fails.
    pub(crate) fn new(oem: &ResourceOemSchema) -> Result<Option<Self>, Error<B>> {
        if oem.additional_properties.get(OEM_KEY).is_none() {
            return Ok(None);
        }
        let oem: DeltaOem =
            serde_json::from_value(oem.additional_properties.clone()).map_err(Error::Json)?;
        Ok(Some(Self {
            data: oem.deltaenergysystems.into(),
            _marker: PhantomData,
        }))
    }

    /// Whether this power supply is currently outputting power.
    ///
    /// Returns `None` when Delta does not report the `Power` flag.
    #[must_use]
    pub fn power(&self) -> Option<bool> {
        self.data.power.and_then(identity)
    }

    /// Target fan speed for this power supply in percent.
    ///
    /// A value of `0` indicates the fan is controlled by the PSU. Returns
    /// `None` when Delta does not report the target.
    #[must_use]
    pub fn fan_speed_target(&self) -> Option<i64> {
        self.data.fan_speed_target.and_then(identity)
    }

    /// Get the raw schema data for this Delta OEM power supply.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<DeltaPowerSupplySchema> {
        self.data.clone()
    }
}

#[derive(Deserialize)]
struct DeltaOem {
    deltaenergysystems: DeltaPowerSupplySchema,
}
