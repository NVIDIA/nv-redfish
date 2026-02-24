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

//! Network adapters
//!

use crate::hardware_id::HardwareIdRef;
use crate::hardware_id::Manufacturer as HardwareIdManufacturer;
use crate::hardware_id::Model as HardwareIdModel;
use crate::hardware_id::PartNumber as HardwareIdPartNumber;
use crate::hardware_id::SerialNumber as HardwareIdSerialNumber;
use crate::schema::redfish::network_adapter::NetworkAdapter as NetworkAdapterSchema;
use crate::schema::redfish::network_adapter_collection::NetworkAdapterCollection as NetworkAdapterCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

/// Network adapters collection.
///
/// Provides functions to access collection members.
pub struct NetworkAdapterCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<NetworkAdapterCollectionSchema>,
}

impl<B: Bmc> NetworkAdapterCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<NetworkAdapterCollectionSchema>,
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
    pub async fn members(&self) -> Result<Vec<NetworkAdapter<B>>, Error<B>> {
        let mut members = Vec::new();
        for m in &self.collection.members {
            members.push(NetworkAdapter::new(&self.bmc, m).await?);
        }
        Ok(members)
    }
}

#[doc(hidden)]
pub enum NetworkAdapterTag {}

/// Network adapter manufacturer.
pub type Manufacturer<T> = HardwareIdManufacturer<T, NetworkAdapterTag>;

/// Network adapter model.
pub type Model<T> = HardwareIdModel<T, NetworkAdapterTag>;

/// Network adapter part number.
pub type PartNumber<T> = HardwareIdPartNumber<T, NetworkAdapterTag>;

/// Network adapter serial number.
pub type SerialNumber<T> = HardwareIdSerialNumber<T, NetworkAdapterTag>;

/// Network Adapter.
///
/// Provides functions to access log entries and perform log operations.
pub struct NetworkAdapter<B: Bmc> {
    data: Arc<NetworkAdapterSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> NetworkAdapter<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<NetworkAdapterSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this ethernet adapter.
    #[must_use]
    pub fn raw(&self) -> Arc<NetworkAdapterSchema> {
        self.data.clone()
    }

    /// Get hardware identifier of the network adpater.
    #[must_use]
    pub fn hardware_id(&self) -> HardwareIdRef<'_, NetworkAdapterTag> {
        HardwareIdRef {
            manufacturer: self
                .data
                .manufacturer
                .as_ref()
                .and_then(Option::as_deref)
                .map(Manufacturer::new),
            model: self
                .data
                .model
                .as_ref()
                .and_then(Option::as_deref)
                .map(Model::new),
            part_number: self
                .data
                .part_number
                .as_ref()
                .and_then(Option::as_deref)
                .map(PartNumber::new),
            serial_number: self
                .data
                .serial_number
                .as_ref()
                .and_then(Option::as_deref)
                .map(SerialNumber::new),
        }
    }
}

impl<B: Bmc> Resource for NetworkAdapter<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
