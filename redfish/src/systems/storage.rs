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

use crate::schema::redfish::storage::Storage as StorageSchema;
use crate::systems::Drive;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

/// Represents a storage controller in a computer system.
///
/// Provides access to storage controller information and associated drives.
pub struct Storage<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<StorageSchema>,
}

impl<B: Bmc> Storage<B> {
    /// Create a new storage handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<StorageSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    /// Get the raw schema data for this storage controller.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<StorageSchema> {
        self.data.clone()
    }

    /// Get drives associated with this storage controller.
    ///
    /// Fetches the drive collection and returns a list of [`Drive`] handles.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage controller does not have drives
    /// - Fetching drive data fails
    pub async fn drives(&self) -> Result<Vec<Drive<B>>, Error<B>> {
        let drives_ref = self
            .data
            .drives
            .as_ref()
            .ok_or(Error::StorageNotAvailable)?;

        let mut drives = Vec::new();
        for d in drives_ref {
            drives.push(Drive::new(&self.bmc, d).await?);
        }

        Ok(drives)
    }
}

impl<B: Bmc> Resource for Storage<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
