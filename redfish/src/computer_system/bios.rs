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
//! Bios

use crate::schema::redfish::bios::Bios as BiosSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::EdmPrimitiveType;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

/// BIOS.
///
/// Provides functions to access BIOS functions.
pub struct Bios<B: Bmc> {
    data: Arc<BiosSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> Bios<B> {
    /// Create a new log bios handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<BiosSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for the BIOS.
    #[must_use]
    pub fn raw(&self) -> Arc<BiosSchema> {
        self.data.clone()
    }

    /// Get bios attribute by key value.
    #[must_use]
    pub fn attribute<'a>(&'a self, name: &str) -> Option<BiosAttributeRef<'a>> {
        self.data
            .attributes
            .as_ref()
            .and_then(|attributes| attributes.dynamic_properties.get(name))
            .map(|v| BiosAttributeRef::new(v.as_ref()))
    }
}

/// Reference to a BIOS attribute.
pub struct BiosAttributeRef<'a> {
    value: Option<&'a EdmPrimitiveType>,
}

impl<'a> BiosAttributeRef<'a> {
    const fn new(value: Option<&'a EdmPrimitiveType>) -> Self {
        Self { value }
    }

    /// Returns true if attribute is null.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.value.is_none()
    }

    /// Returns string value of the attribute if attribute is string.
    #[must_use]
    pub const fn str_value(&self) -> Option<&str> {
        match self.value {
            Some(EdmPrimitiveType::String(v)) => Some(v.as_str()),
            _ => None,
        }
    }
}
