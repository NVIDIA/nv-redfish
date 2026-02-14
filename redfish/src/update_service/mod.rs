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

use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::update_service::UpdateService as UpdateServiceSchema;
use crate::schema::redfish::update_service::UpdateServiceSimpleUpdateAction;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use crate::ServiceRoot;
use nv_redfish_core::Bmc;
use serde_json::Value as JsonValue;
use software_inventory::SoftwareInventoryCollection;
use std::sync::Arc;

#[doc(inline)]
pub use crate::schema::redfish::update_service::TransferProtocolType;
#[doc(inline)]
pub use software_inventory::SoftwareInventory;
#[doc(inline)]
pub use software_inventory::Version;
#[doc(inline)]
pub use software_inventory::VersionRef;

/// Update service.
///
/// Provides functions to access firmware and software inventory, and perform update actions.
pub struct UpdateService<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<UpdateServiceSchema>,
    fw_inventory_read_patch_fn: Option<ReadPatchFn>,
}

impl<B: Bmc> UpdateService<B> {
    /// Create a new update service handle.
    pub(crate) async fn new(bmc: &NvBmc<B>, root: &ServiceRoot<B>) -> Result<Self, Error<B>> {
        let service_ref = root
            .root
            .update_service
            .as_ref()
            .ok_or(Error::UpdateServiceNotSupported)?;
        let data = service_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

        let mut fw_inventory_patches = Vec::new();
        if root.fw_inventory_wrong_release_date() {
            fw_inventory_patches.push(fw_inventory_patch_wrong_release_date);
        }
        let fw_inventory_read_patch_fn = if fw_inventory_patches.is_empty() {
            None
        } else {
            let fw_inventory_patches_fn: ReadPatchFn =
                Arc::new(move |v| fw_inventory_patches.iter().fold(v, |acc, f| f(acc)));
            Some(fw_inventory_patches_fn)
        };
        Ok(Self {
            bmc: bmc.clone(),
            data,
            fw_inventory_read_patch_fn,
        })
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
    pub async fn firmware_inventories(&self) -> Result<Vec<SoftwareInventory<B>>, Error<B>> {
        let collection_ref = self
            .data
            .firmware_inventory
            .as_ref()
            .ok_or(Error::FirmwareInventoryNotAvailable)?;

        SoftwareInventoryCollection::new(
            &self.bmc,
            collection_ref,
            self.fw_inventory_read_patch_fn.clone(),
        )
        .await?
        .members()
        .await
    }

    /// List all software inventory items.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not have a software inventory collection
    /// - Fetching software inventory data fails
    pub async fn software_inventories(&self) -> Result<Vec<SoftwareInventory<B>>, Error<B>> {
        let collection_ref = self
            .data
            .software_inventory
            .as_ref()
            .ok_or(Error::SoftwareInventoryNotAvailable)?;
        let collection = self.bmc.expand_property(collection_ref).await?;
        let mut items = Vec::new();
        for item_ref in &collection.members {
            items.push(SoftwareInventory::new(&self.bmc, item_ref, None).await?);
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
    /// * `local_image` - An indication of whether the service adds the image to the local image store
    /// * `exclude_targets` - An array of URIs that indicate where not to apply the update image
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not support the `SimpleUpdate` action
    /// - The action execution fails
    #[allow(clippy::too_many_arguments)]
    pub async fn simple_update(
        &self,
        image_uri: String,
        transfer_protocol: Option<TransferProtocolType>,
        targets: Option<Vec<String>>,
        username: Option<String>,
        password: Option<String>,
        force_update: Option<bool>,
        stage: Option<bool>,
        local_image: Option<bool>,
        exclude_targets: Option<Vec<String>>,
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
                    local_image,
                    exclude_targets,
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
    /// - The update service does not support the `StartUpdate` action
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

impl<B: Bmc> Resource for UpdateService<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}

// `ReleaseDate` is marked as `edm.DateTimeOffset`, but some systems
// puts "00:00:00Z" as ReleaseDate that is not conform to ABNF of the DateTimeOffset.
// we delete such fields...
fn fw_inventory_patch_wrong_release_date(v: JsonValue) -> JsonValue {
    if let JsonValue::Object(mut obj) = v {
        if let Some(JsonValue::String(date)) = obj.get("ReleaseDate") {
            if date == "00:00:00Z" {
                obj.remove("ReleaseDate");
            }
        }
        JsonValue::Object(obj)
    } else {
        v
    }
}
