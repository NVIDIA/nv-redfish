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

use std::sync::Arc;

use nv_redfish_core::Bmc;

use crate::schema::redfish::software_inventory::SoftwareInventory as SoftwareInventorySchema;

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
    pub(crate) fn new(bmc: Arc<B>, data: Arc<SoftwareInventorySchema>) -> Self {
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

