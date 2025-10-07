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
use crate::Creatable;
use crate::Deletable;
use crate::EntityTypeRef;
use crate::Expandable;
use crate::ODataETag;
use crate::ODataId;
use crate::Updatable;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::sync::Arc;

/// Reference varian of the navigation property (only `@odata.id`
/// property specified).
#[derive(Serialize, Deserialize, Debug)]
pub struct Reference {
    #[serde(rename = "@odata.id")]
    pub odata_id: ODataId,
}

/// Container struct for expanded property variant
#[derive(Debug)]
pub struct Expanded<T>(Arc<T>);

/// Deserializer wraps Expanded property into Arc
impl<'de, T> Deserialize<'de> for Expanded<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Arc::new).map(Expanded)
    }
}

/// Navigation property variants. All navigation properties in
/// generated code are wrapped with this type.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum NavProperty<T: EntityTypeRef> {
    /// Expanded property variant (content included into the
    /// response).
    Expanded(Expanded<T>),
    /// Reference variant (only `@odata.id` is included into the
    /// response).
    Reference(Reference),
}

impl<T: EntityTypeRef> EntityTypeRef for NavProperty<T> {
    fn id(&self) -> &ODataId {
        match self {
            Self::Expanded(v) => v.0.id(),
            Self::Reference(r) => &r.odata_id,
        }
    }

    fn etag(&self) -> Option<&ODataETag> {
        match self {
            Self::Expanded(v) => v.0.etag(),
            Self::Reference(_) => None,
        }
    }
}

impl<C, R, T: Creatable<C, R>> Creatable<C, R> for NavProperty<T>
where
    C: Sync + Send + Sized + Serialize,
    R: Sync + Send + Sized + for<'de> Deserialize<'de>,
{
}
impl<U, T: Updatable<U>> Updatable<U> for NavProperty<T> where U: Sync + Send + Sized + Serialize {}
impl<T: Deletable> Deletable for NavProperty<T> {}
impl<T: Expandable> Expandable for NavProperty<T> {}

impl<T: EntityTypeRef> NavProperty<T> {
    pub fn new_reference(odata_id: ODataId) -> Self {
        Self::Reference(Reference { odata_id })
    }
}

impl<T: EntityTypeRef> NavProperty<T> {
    /// Extract identifier from navigation property.
    pub fn id(&self) -> &ODataId {
        match self {
            Self::Reference(v) => &v.odata_id,
            Self::Expanded(v) => v.0.id(),
        }
    }
}

impl<T: EntityTypeRef + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync> NavProperty<T> {
    /// Get property
    pub async fn get<B: Bmc>(&self, bmc: &B) -> Result<Arc<T>, B::Error> {
        match self {
            Self::Expanded(v) => Ok(v.0.clone()),
            Self::Reference(_) => bmc.get::<T>(self.id()).await,
        }
    }
}
