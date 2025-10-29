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

pub use manager::Manager;

use std::sync::Arc;

use nv_redfish_core::{http::ExpandQuery, Bmc, Expandable, NavProperty};

use crate::schema::redfish::manager_collection::ManagerCollection as ManagerCollectionSchema;
use crate::Error;

/// Manager collection.
///
/// Provides functions to access collection members.
pub struct ManagerCollection<B: Bmc> {
    bmc: Arc<B>,
    collection: Arc<ManagerCollectionSchema>,
}

impl<B: Bmc + Sync + Send> ManagerCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(
        bmc: Arc<B>,
        collection_ref: &NavProperty<ManagerCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = collection_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

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
    pub async fn list_managers(&self) -> Result<Vec<Manager<B>>, Error<B>> {
        let mut managers = Vec::new();
        for manager_ref in &self
            .collection
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .members
        {
            let manager = manager_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            managers.push(Manager::new(self.bmc.clone(), manager));
        }

        Ok(managers)
    }
}


