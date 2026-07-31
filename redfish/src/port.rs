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

//! Network ports.

use crate::mac_address::MacAddress;
#[cfg(feature = "oem-lenovo")]
use crate::oem::lenovo::port::LenovoPort;
use crate::schema::port::Port as PortSchema;
use crate::schema::port_collection::PortCollection as PortCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

/// Network port collection.
///
/// Provides functions to access collection members.
pub struct PortCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<PortCollectionSchema>,
}

impl<B: Bmc> PortCollection<B> {
    #[allow(dead_code)] // Used by NetworkAdapter when that feature is enabled.
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
}

/// Network port.
pub struct Port<B: Bmc> {
    data: Arc<PortSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> Port<B> {
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<PortSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this port.
    #[must_use]
    pub fn raw(&self) -> Arc<PortSchema> {
        self.data.clone()
    }

    /// Get MAC addresses associated with this port.
    ///
    /// Standard `Ethernet.AssociatedMACAddresses` values are authoritative.
    /// Lenovo XCC reports the physical address only through
    /// `Oem.Lenovo.PhysicalPortMacAddress` on some systems, so that value is
    /// returned when the standard array is absent or empty.
    ///
    #[must_use]
    pub fn associated_mac_addresses(&self) -> Vec<MacAddress<'_>> {
        let standard = self
            .data
            .ethernet
            .as_ref()
            .and_then(Option::as_ref)
            .and_then(|ethernet| ethernet.associated_mac_addresses.as_ref())
            .and_then(Option::as_ref)
            .filter(|addresses| !addresses.is_empty());

        if let Some(addresses) = standard {
            return addresses
                .iter()
                .map(String::as_str)
                .map(MacAddress::new)
                .collect();
        }

        #[cfg(feature = "oem-lenovo")]
        if let Some(address) = LenovoPort::new(&self.data)
            .as_ref()
            .and_then(LenovoPort::physical_port_mac_address)
        {
            return vec![MacAddress::new(address)];
        }

        Vec::new()
    }
}

impl<B: Bmc> Resource for Port<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
