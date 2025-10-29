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

//! Update Service entities and collections.
//!
//! This module provides types for working with Redfish UpdateService resources
//! and their sub-resources like firmware and software inventory.

mod software_inventory;

pub use software_inventory::SoftwareInventory;

// Re-export types needed for actions
pub use crate::schema::redfish::update_service::TransferProtocolType;

use std::sync::Arc;

use nv_redfish_core::{http::ExpandQuery, Bmc, Expandable};

use crate::schema::redfish::update_service::{
    UpdateService as UpdateServiceSchema, UpdateServiceSimpleUpdateAction,
};
use crate::Error;

/// Update service.
///
/// Provides functions to access firmware and software inventory, and perform update actions.
pub struct UpdateService<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<UpdateServiceSchema>,
}

impl<B: Bmc + Sync + Send> UpdateService<B> {
    /// Create a new update service handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<UpdateServiceSchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this update service.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<UpdateServiceSchema> {
        self.data.clone()
    }

    /// List all firmware inventory items.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not have a firmware inventory collection
    /// - Fetching firmware inventory data fails
    pub async fn list_firmware_inventory(&self) -> Result<Vec<SoftwareInventory<B>>, Error<B>> {
        let collection_ref = self
            .data
            .firmware_inventory
            .as_ref()
            .ok_or(Error::FirmwareInventoryNotAvailable)?;

        let collection = collection_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut items = Vec::new();
        for item_ref in &collection.members {
            let item = item_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            items.push(SoftwareInventory::new(self.bmc.clone(), item));
        }
        Ok(items)
    }

    /// List all software inventory items.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not have a software inventory collection
    /// - Fetching software inventory data fails
    pub async fn list_software_inventory(&self) -> Result<Vec<SoftwareInventory<B>>, Error<B>> {
        let collection_ref = self
            .data
            .software_inventory
            .as_ref()
            .ok_or(Error::SoftwareInventoryNotAvailable)?;

        let collection = collection_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut items = Vec::new();
        for item_ref in &collection.members {
            let item = item_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            items.push(SoftwareInventory::new(self.bmc.clone(), item));
        }
        Ok(items)
    }

    /// Perform a simple update with the specified image URI.
    ///
    /// This action updates software components by downloading and installing
    /// a software image from the specified URI.
    ///
    /// # Arguments
    ///
    /// * `image_uri` - The URI of the software image to install
    /// * `transfer_protocol` - Optional network protocol to use for retrieving the image
    /// * `targets` - Optional list of URIs indicating where to apply the update
    /// * `username` - Optional username for accessing the image URI
    /// * `password` - Optional password for accessing the image URI
    /// * `force_update` - Whether to bypass update policies (e.g., allow downgrade)
    /// * `stage` - Whether to stage the image for later activation instead of immediate installation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not support the SimpleUpdate action
    /// - The action execution fails
    pub async fn simple_update(
        &self,
        image_uri: String,
        transfer_protocol: Option<TransferProtocolType>,
        targets: Option<Vec<String>>,
        username: Option<String>,
        password: Option<String>,
        force_update: Option<bool>,
        stage: Option<bool>,
    ) -> Result<(), Error<B>>
    where
        B::Error: nv_redfish_core::ActionError,
    {
        let actions = self
            .data
            .actions
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;

        actions
            .simple_update(
                self.bmc.as_ref(),
                &UpdateServiceSimpleUpdateAction {
                    image_uri: Some(image_uri),
                    transfer_protocol,
                    targets,
                    username,
                    password,
                    force_update,
                    stage,
                },
            )
            .await
            .map_err(Error::Bmc)?;

        Ok(())
    }

    /// Start updates that have been previously invoked with an `OperationApplyTime` of `OnStartUpdateRequest`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not support the StartUpdate action
    /// - The action execution fails
    pub async fn start_update(&self) -> Result<(), Error<B>>
    where
        B::Error: nv_redfish_core::ActionError,
    {
        let actions = self
            .data
            .actions
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;

        actions
            .start_update(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        Ok(())
    }
}

