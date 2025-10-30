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

//! Computer System entities and collections.
//!
//! This module provides types for working with Redfish ComputerSystem resources
//! and their sub-resources like processors, storage, memory, and drives.

mod computer_system;

#[cfg(feature = "storages")]
mod drive;
#[cfg(feature = "memory")]
mod memory;
#[cfg(feature = "processors")]
mod processor;
#[cfg(feature = "storages")]
mod storage;

use crate::schema::redfish::computer_system_collection::ComputerSystemCollection as ComputerSystemCollectionSchema;
use crate::Error;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Bmc;
use nv_redfish_core::Expandable as _;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

#[doc(inline)]
pub use computer_system::ComputerSystem;
#[doc(inline)]
#[cfg(feature = "storages")]
pub use drive::Drive;
#[doc(inline)]
#[cfg(feature = "memory")]
pub use memory::Memory;
#[doc(inline)]
#[cfg(feature = "processors")]
pub use processor::Processor;
#[doc(inline)]
#[cfg(feature = "storages")]
pub use storage::Storage;

/// Computer system collection.
///
/// Provides functions to access collection members.
pub struct SystemCollection<B: Bmc> {
    bmc: Arc<B>,
    collection: Arc<ComputerSystemCollectionSchema>,
}

impl<B: Bmc + Sync + Send> SystemCollection<B> {
    pub(crate) async fn new(
        bmc: Arc<B>,
        collection_ref: &NavProperty<ComputerSystemCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = collection_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all computer systems available in this BMC.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching system data fails.
    pub async fn systems(&self) -> Result<Vec<ComputerSystem<B>>, Error<B>> {
        let mut systems = Vec::new();
        for system_ref in &self
            .collection
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .members
        {
            let system = system_ref
                .get(self.bmc.as_ref())
                .await
                .map_err(Error::Bmc)?;
            systems.push(ComputerSystem::new(self.bmc.clone(), system));
        }

        Ok(systems)
    }
}
