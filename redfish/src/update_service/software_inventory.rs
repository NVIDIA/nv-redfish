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

use crate::patch_support::CollectionWithPatch;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::resource::ResourceCollection;
use crate::schema::redfish::software_inventory::SoftwareInventory as SoftwareInventorySchema;
use crate::schema::redfish::software_inventory_collection::SoftwareInventoryCollection as SoftwareInventoryCollectionSchema;
use crate::Error;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

/// Represents a software inventory item in the update service.
///
/// Provides access to software version information and metadata.
pub struct SoftwareInventory<B: Bmc> {
    #[allow(dead_code)]
    bmc: Arc<B>,
    data: Arc<SoftwareInventorySchema>,
}

impl<B: Bmc> SoftwareInventory<B> {
    /// Create a new software inventory handle.
    pub(crate) const fn new(bmc: Arc<B>, data: Arc<SoftwareInventorySchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this software inventory item.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<SoftwareInventorySchema> {
        self.data.clone()
    }
}

pub struct SoftwareInventoryCollection<B: Bmc> {
    bmc: Arc<B>,
    collection: Arc<SoftwareInventoryCollectionSchema>,
    read_patch_fn: Option<ReadPatchFn>,
}

impl<B: Bmc> CollectionWithPatch<SoftwareInventoryCollectionSchema, SoftwareInventorySchema, B>
    for SoftwareInventoryCollection<B>
{
    fn convert_patched(
        base: ResourceCollection,
        members: Vec<NavProperty<SoftwareInventorySchema>>,
    ) -> SoftwareInventoryCollectionSchema {
        SoftwareInventoryCollectionSchema { base, members }
    }
}

impl<B: Bmc> SoftwareInventoryCollection<B> {
    pub(crate) async fn new(
        bmc: Arc<B>,
        collection_ref: &NavProperty<SoftwareInventoryCollectionSchema>,
        read_patch_fn: Option<ReadPatchFn>,
    ) -> Result<Self, Error<B>> {
        let query = ExpandQuery::all();
        let collection =
            Self::read_collection(bmc.as_ref(), collection_ref, read_patch_fn.as_ref(), query)
                .await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
            read_patch_fn,
        })
    }

    pub(crate) async fn members(&self) -> Result<Vec<SoftwareInventory<B>>, Error<B>> {
        let mut items = Vec::new();
        for nav in &self.collection.members {
            let item = self.get_one(nav).await?;
            items.push(SoftwareInventory::new(self.bmc.clone(), item));
        }
        Ok(items)
    }

    async fn get_one(
        &self,
        nav: &NavProperty<SoftwareInventorySchema>,
    ) -> Result<Arc<SoftwareInventorySchema>, Error<B>> {
        if let Some(read_patch_fn) = &self.read_patch_fn {
            Payload::get(self.bmc.as_ref(), nav, read_patch_fn.as_ref()).await
        } else {
            nav.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
        }
    }
}
