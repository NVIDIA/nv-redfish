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

//! Ethernet interfaces
//!

use crate::schema::redfish::ethernet_interface::EthernetInterface as EthernetInterfaceSchema;
use crate::schema::redfish::ethernet_interface_collection::EthernetInterfaceCollection as EthernetInterfaceCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

/// Ethernet interfaces collection.
///
/// Provides functions to access collection members.
pub struct EthernetInterfaceCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<EthernetInterfaceCollectionSchema>,
}

impl<B: Bmc> EthernetInterfaceCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<EthernetInterfaceCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = bmc.expand_property(nav).await?;
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
    pub async fn members(&self) -> Result<Vec<EthernetInterface<B>>, Error<B>> {
        let mut members = Vec::new();
        for m in &self.collection.members {
            members.push(EthernetInterface::new(&self.bmc, m).await?);
        }
        Ok(members)
    }
}

/// Ethernet Interface.
///
/// Provides functions to access log entries and perform log operations.
pub struct EthernetInterface<B: Bmc> {
    data: Arc<EthernetInterfaceSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> EthernetInterface<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<EthernetInterfaceSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this ethernet interface.
    #[must_use]
    pub fn raw(&self) -> Arc<EthernetInterfaceSchema> {
        self.data.clone()
    }
}

impl<B: Bmc> Resource for EthernetInterface<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
