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
mod drive;
mod memory;
mod processor;
mod storage;

pub use computer_system::ComputerSystem;
pub use drive::Drive;
pub use memory::Memory;
pub use processor::Processor;
pub use storage::Storage;

use std::sync::Arc;

use nv_redfish_core::{http::ExpandQuery, Bmc, Expandable, NavProperty};

use crate::schema::redfish::computer_system_collection::ComputerSystemCollection as ComputerSystemCollectionSchema;
use crate::Error;

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
    pub async fn list_systems(&self) -> Result<Vec<ComputerSystem<B>>, Error<B>> {
        let mut systems = Vec::new();
        for system_ref in &self
            .collection
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .members
        {
            let system = system_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            systems.push(ComputerSystem::new(self.bmc.clone(), system));
        }

        Ok(systems)
    }
}

