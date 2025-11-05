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

//! Manager entities and collections.
//!
//! This module provides types for working with Redfish Manager resources.

mod manager;

use crate::schema::redfish::manager_collection::ManagerCollection as ManagerCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::ServiceRoot;
use nv_redfish_core::Bmc;
use std::sync::Arc;

pub use manager::Manager;

/// Manager collection.
///
/// Provides functions to access collection members.
pub struct ManagerCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<ManagerCollectionSchema>,
}

impl<B: Bmc + Sync + Send> ManagerCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(bmc: &NvBmc<B>, root: &ServiceRoot<B>) -> Result<Self, Error<B>> {
        let collection_ref = root
            .root
            .managers
            .as_ref()
            .ok_or(Error::ManagerNotSupported)?;

        let collection = bmc.expand_property(collection_ref).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all managers available in this BMC.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching manager data fails.
    pub async fn managers(&self) -> Result<Vec<Manager<B>>, Error<B>> {
        let mut managers = Vec::new();
        for m in &self.collection.members {
            managers.push(Manager::new(&self.bmc, m).await?);
        }
        Ok(managers)
    }
}
