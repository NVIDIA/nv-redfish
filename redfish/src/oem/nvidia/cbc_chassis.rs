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

//! Support NVIDIA CBC Chassis OEM extension.

use crate::oem::declares;
use crate::oem::nvidia::schema::nvidia_chassis::NvidiaCbcChassis as NvidiaCbcChassisSchema;
use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use nv_redfish_core::Bmc;
use serde::Deserialize as _;
use std::convert::identity;
use std::marker::PhantomData;
use std::sync::Arc;
use tagged_types::TaggedType;

/// Namespace the CBC chassis shape is declared under.
const NAMESPACE: &str = "NvidiaChassis";
/// `@odata.type` name of the CBC chassis shape.
const TYPE_NAME: &str = "NvidiaCBCChassis";

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
    /// Create a new CBC chassis handle.
    ///
    /// Returns `Ok(None)` when the OEM payload does not contain NVIDIA CBC chassis data.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing NVIDIA CBC chassis OEM data fails.
    pub(crate) fn new(oem: &ResourceOemSchema) -> Result<Option<Self>, Error<B>> {
        let Some(nvidia) = oem_value(oem, OEM_KEY) else {
            return Ok(None);
        };
        if !declares(nvidia, NAMESPACE, TYPE_NAME) {
            return Ok(None);
        }
        Ok(Some(Self {
            data: Arc::new(NvidiaCbcChassisSchema::deserialize(nvidia).map_err(Error::Json)?),
            _marker: PhantomData,
        }))
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

    /// Get the raw schema data for this NVIDIA CBC chassis.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<NvidiaCbcChassisSchema> {
        self.data.clone()
    }
}
