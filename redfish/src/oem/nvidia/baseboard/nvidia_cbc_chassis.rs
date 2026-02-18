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

//! Support NVIDIA Baseboard Chassis OEM extension.

use crate::oem::nvidia::baseboard::schema::redfish::nvidia_chassis::NvidiaCbcChassis as NvidiaCbcChassisSchema;
use crate::schema::redfish::resource::Oem as ResourceOemSchema;
use crate::Error;
use nv_redfish_core::odata::ODataType;
use nv_redfish_core::Bmc;
use serde::Deserialize;
use std::convert::identity;
use std::marker::PhantomData;
use std::sync::Arc;
use tagged_types::TaggedType;

/// The revision of the cable cartridge backplane FRU data information.
pub type RevisionId = TaggedType<i64, RevisionIdTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum RevisionIdTag {}

/// The chassis physical slot Number of the compute tray.
pub type ChassisPhysicalSlotNumber = TaggedType<i64, ChassisPhysicalSlotNumberTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ChassisPhysicalSlotNumberTag {}

/// The compute tray index within the chassis.
pub type ComputeTrayIndex = TaggedType<i64, ComputeTrayIndexTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ComputeTrayIndexTag {}

/// The topology of the chassis.
pub type TopologyId = TaggedType<i64, TopologyIdTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum TopologyIdTag {}

/// Represents a NVIDIA extension of CBC chassis in the BMC.
///
/// Provides access to system information and sub-resources such as processors.
pub struct NvidiaCbcChassis<B: Bmc> {
    data: Arc<NvidiaCbcChassisSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> NvidiaCbcChassis<B> {
    /// Create a new computer system handle.
    pub(crate) fn new(oem: &ResourceOemSchema) -> Result<Self, Error<B>> {
        let is_cbc_chassis = oem
            .additional_properties
            .get("Nvidia")
            .and_then(ODataType::parse_from)
            .and_then(|odata_type| {
                let type_name = odata_type.type_name;
                odata_type
                    .namespace
                    .into_iter()
                    .next()
                    .map(|ns| (ns, type_name))
            })
            .map(|(top_ns, t)| top_ns == "NvidiaChassis" && t == "NvidiaCBCChassis");
        if is_cbc_chassis.is_some_and(identity) {
            let oem: CbcOem =
                serde_json::from_value(oem.additional_properties.clone()).map_err(Error::Json)?;
            Ok(Self {
                data: oem.nvidia.into(),
                _marker: PhantomData,
            })
        } else {
            Err(Error::NvidiaCbcChassisNotAvailable)
        }
    }

    /// Indicates the revision of the cable cartridge backplane FRU data information.
    pub fn revision_id(&self) -> Option<RevisionId> {
        self.data
            .revision_id
            .and_then(identity)
            .map(RevisionId::new)
    }

    /// Indicates the chassis physical slot Number of the compute tray.
    pub fn chassis_physical_slot_number(&self) -> Option<ChassisPhysicalSlotNumber> {
        self.data
            .chassis_physical_slot_number
            .and_then(identity)
            .map(ChassisPhysicalSlotNumber::new)
    }

    /// Indicates the compute tray index within the chassis.
    pub fn compute_tray_index(&self) -> Option<ComputeTrayIndex> {
        self.data
            .compute_tray_index
            .and_then(identity)
            .map(ComputeTrayIndex::new)
    }

    /// Indicates the topology of the chassis.
    pub fn topology_id(&self) -> Option<TopologyId> {
        self.data
            .topology_id
            .and_then(identity)
            .map(TopologyId::new)
    }

    /// Get the raw schema data for this NVIDIA computer system.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<NvidiaCbcChassisSchema> {
        self.data.clone()
    }
}

#[derive(Deserialize)]
struct CbcOem {
    #[serde(rename = "Nvidia")]
    nvidia: NvidiaCbcChassisSchema,
}
