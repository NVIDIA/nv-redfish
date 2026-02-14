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

//! Assembly
//!

use crate::hardware_id::HardwareIdRef;
use crate::hardware_id::Manufacturer as HardwareIdManufacturer;
use crate::hardware_id::Model as HardwareIdModel;
use crate::hardware_id::PartNumber as HardwareIdPartNumber;
use crate::hardware_id::SerialNumber as HardwareIdSerialNumber;
use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::assembly::Assembly as AssemblySchema;
use crate::schema::redfish::assembly::AssemblyData as AssemblyDataSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use crate::ServiceRoot;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

#[doc(hidden)]
pub enum AssemblyTag {}

/// Assembly manufacturer (AKA Producer).
pub type Manufacturer<T> = HardwareIdManufacturer<T, AssemblyTag>;

/// Assembly model.
pub type Model<T> = HardwareIdModel<T, AssemblyTag>;

/// Assembly part number.
pub type PartNumber<T> = HardwareIdPartNumber<T, AssemblyTag>;

/// Assembly number.
pub type SerialNumber<T> = HardwareIdSerialNumber<T, AssemblyTag>;

/// Configuration of the Assembly.
pub struct Config {
    read_patch_fn: Option<ReadPatchFn>,
}

impl Config {
    /// New configuration of the assembly from parametes of the
    /// service root.
    pub fn new<B: Bmc>(root: &ServiceRoot<B>) -> Self {
        let mut patches = Vec::new();
        if root.assembly_assemblies_without_odata_type() {
            patches.push(add_odata_type_to_assemblies);
        }
        let read_patch_fn = if patches.is_empty() {
            None
        } else {
            let read_patch_fn: ReadPatchFn =
                Arc::new(move |v| patches.iter().fold(v, |acc, f| f(acc)));
            Some(read_patch_fn)
        };
        Self { read_patch_fn }
    }
}

/// Assembly.
///
/// Provides functions to access assembly.
pub struct Assembly<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<AssemblySchema>,
}

impl<B: Bmc> Assembly<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<AssemblySchema>,
        config: &Config,
    ) -> Result<Self, Error<B>> {
        // We use expand here becuase Assembly/Assemblies are
        // navigation properties, so we want to take them using one
        // get.
        if let Some(read_patch_fn) = &config.read_patch_fn {
            Payload::expand_property(bmc, nav, read_patch_fn.as_ref()).await
        } else {
            bmc.expand_property(nav).await
        }
        .map(|data| Self {
            bmc: bmc.clone(),
            data,
        })
    }

    /// Get the raw schema data for this assembly.
    #[must_use]
    pub fn raw(&self) -> Arc<AssemblySchema> {
        self.data.clone()
    }

    /// Get assemblies.
    ///
    /// # Errors
    ///
    /// Returns error if this assembly was not expanded by initial get
    /// and then function failed to get data of the assembly.
    pub async fn assemblies(&self) -> Result<Vec<AssemblyData<B>>, Error<B>> {
        let mut result = Vec::new();
        if let Some(assemblies) = &self.data.assemblies {
            for m in assemblies {
                result.push(AssemblyData::new(&self.bmc, m).await?);
            }
        }
        Ok(result)
    }
}

impl<B: Bmc> Resource for Assembly<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}

/// Assembly data.
pub struct AssemblyData<B: Bmc> {
    data: Arc<AssemblyDataSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> AssemblyData<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<AssemblyDataSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this assembly.
    #[must_use]
    pub fn raw(&self) -> Arc<AssemblyDataSchema> {
        self.data.clone()
    }

    /// Get hardware identifier of the network adpater.
    #[must_use]
    pub fn hardware_id(&self) -> HardwareIdRef<'_, AssemblyTag> {
        HardwareIdRef {
            manufacturer: self
                .data
                .producer
                .as_ref()
                .and_then(Option::as_ref)
                .map(Manufacturer::new),
            model: self
                .data
                .model
                .as_ref()
                .and_then(Option::as_ref)
                .map(Model::new),
            part_number: self
                .data
                .part_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(PartNumber::new),
            serial_number: self
                .data
                .serial_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(SerialNumber::new),
        }
    }
}

fn add_odata_type_to_assemblies(mut v: JsonValue) -> JsonValue {
    if let Some(assemblies) = v
        .as_object_mut()
        .and_then(|obj| obj.get_mut("Assemblies"))
        .and_then(|assemblies| assemblies.as_array_mut())
    {
        for assembly in assemblies.iter_mut() {
            if let Some(obj) = assembly.as_object_mut() {
                if obj.len() > 1 && !obj.contains_key("@odata.type") {
                    obj.insert(
                        "@odata.type".to_string(),
                        "#Assembly.v1_5_1.AssemblyData".into(),
                    );
                }
            }
        }
    }
    v
}
