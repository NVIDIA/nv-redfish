// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use super::Port;
use super::PortLink;
use crate::schema::port_collection::PortCollection as PortCollectionSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

/// Network port collection.
///
/// Provides functions to access collection members.
pub struct PortCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<PortCollectionSchema>,
}

impl<B: Bmc> PortCollection<B> {
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<PortCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = bmc.expand_property(nav).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all ports in this collection.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching port data fails.
    pub async fn members(&self) -> Result<Vec<Port<B>>, Error<B>> {
        let mut members = Vec::new();
        for member in &self.collection.members {
            members.push(Port::new(&self.bmc, member).await?);
        }
        Ok(members)
    }

    /// Return lazy links for the ports in this collection.
    ///
    /// Each link can be fetched independently, allowing callers to choose their own error-handling
    /// policy without changing the eager, all-or-nothing behavior of [`Self::members`].
    #[must_use]
    pub fn member_links(&self) -> Vec<PortLink<B>> {
        self.collection
            .members
            .iter()
            .map(|member| PortLink::new(&self.bmc, NavProperty::new_reference(member.id().clone())))
            .collect()
    }
}
