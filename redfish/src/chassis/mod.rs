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

#[allow(clippy::module_inception)]
mod chassis;

#[cfg(feature = "power")]
mod power;
#[cfg(feature = "power-supplies")]
mod power_supply;
#[cfg(feature = "thermal")]
mod thermal;

use crate::schema::redfish::chassis_collection::ChassisCollection as ChassisCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::ServiceRoot;
use nv_redfish_core::Bmc;
use std::sync::Arc;

#[doc(inline)]
pub use chassis::Chassis;

#[doc(inline)]
#[cfg(feature = "power")]
pub use power::Power;
#[doc(inline)]
#[cfg(feature = "power-supplies")]
pub use power_supply::PowerSupply;
#[doc(inline)]
#[cfg(feature = "thermal")]
pub use thermal::Thermal;

/// Chassis collection.
///
/// Provides functions to access collection members.
pub struct ChassisCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<ChassisCollectionSchema>,
}

impl<B: Bmc + Sync + Send> ChassisCollection<B> {
    pub(crate) async fn new(bmc: &NvBmc<B>, root: &ServiceRoot<B>) -> Result<Self, Error<B>> {
        let collection_ref = root
            .root
            .chassis
            .as_ref()
            .ok_or(Error::ChassisNotSupported)?;

        let collection = bmc.expand_property(collection_ref).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all chassis avaiable in this BMC
    ///
    /// # Errors
    ///
    /// Returns an error if fetching collection data fails.
    pub async fn chassis(&self) -> Result<Vec<Chassis<B>>, Error<B>> {
        let mut chassis_members = Vec::new();
        for chassis in &self.collection.members {
            chassis_members.push(Chassis::new(&self.bmc, chassis).await?);
        }

        Ok(chassis_members)
    }
}
