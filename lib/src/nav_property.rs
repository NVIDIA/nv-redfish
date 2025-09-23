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

use crate::Bmc;
use crate::EntityType;
use crate::Expandable;
use crate::ODataId;
use serde::Deserialize;
use serde::Serialize;

/// Reference varian of the navigation property (only `@odata.id`
/// property specified).
#[derive(Serialize, Deserialize, Debug)]
pub struct Reference {
    #[serde(rename = "@odata.id")]
    pub odata_id: ODataId,
}

/// Navigation property variants. All navigation properties in
/// generated code are wrapped with this type.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum NavProperty<T: EntityType> {
    /// Expanded property variant (content included into the
    /// response).
    Expanded(T),
    /// Reference variant (only `@odata.id` is included into the
    /// response).
    Reference(Reference),
}

impl<T: EntityType> NavProperty<T> {
    pub fn new_reference(odata_id: ODataId) -> Self {
        Self::Reference(Reference { odata_id })
    }
}

impl<T: EntityType> NavProperty<T> {
    /// Extract identifier from navigation property.
    pub fn id(&self) -> &ODataId {
        match self {
            Self::Reference(v) => &v.odata_id,
            Self::Expanded(v) => v.id(),
        }
    }
}

impl<T: Expandable> NavProperty<T> {
    /// Expand property
    pub async fn expand<B: Bmc>(self, bmc: &B) -> Result<T, B::Error> {
        match self {
            Self::Expanded(v) => Ok(v),
            Self::Reference(_) => bmc.expand::<T>(self.id()).await,
        }
    }
}
