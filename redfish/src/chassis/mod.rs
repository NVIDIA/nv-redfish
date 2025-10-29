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

mod chassis;
mod power;
mod power_supply;
mod thermal;

pub use chassis::Chassis;
pub use power::Power;
pub use power_supply::PowerSupply;
pub use thermal::Thermal;

use std::sync::Arc;

use crate::schema::redfish::chassis_collection::ChassisCollection as ChassisCollectionSchema;
use crate::Error;
use nv_redfish_core::http::ExpandQuery;
use nv_redfish_core::{Bmc, Expandable, NavProperty};

/// Chassis collection.
///
/// Provides functions to access collection members.
pub struct ChassisCollection<B: Bmc> {
    bmc: Arc<B>,
    collection: Arc<ChassisCollectionSchema>,
}

impl<B: Bmc + Sync + Send> ChassisCollection<B> {
    pub(crate) async fn new(
        bmc: Arc<B>,
        collection_ref: &NavProperty<ChassisCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = collection_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all chassis avaiable in this BMC
    pub async fn list_chassis(&self) -> Result<Vec<Chassis<B>>, Error<B>> {
        let mut chassis_members = Vec::new();
        for chassis in &self
            .collection
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .members
        {
            let chassis = chassis.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            chassis_members.push(Chassis::new(self.bmc.clone(), chassis));
        }

        Ok(chassis_members)
    }
}
