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

use crate::patch_support::JsonValue;
use crate::Error;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use serde::Deserializer;
use std::sync::Arc;

#[cfg(feature = "patch-payload-update")]
use crate::patch_support::ReadPatchFn;
#[cfg(feature = "patch-payload-update")]
use nv_redfish_core::Updatable;
#[cfg(feature = "patch-payload-update")]
use serde::Serialize;

#[cfg(feature = "patch-payload-expand")]
use crate::NvBmc;

#[cfg(feature = "patch-payload-update")]
pub trait UpdateWithPatch<T, V, B>
where
    V: Serialize + Send + Sync,
    T: EntityTypeRef + Updatable<V> + Sync + Send,
    B: Bmc,
{
    fn entity_ref(&self) -> &T;
    fn patch(&self) -> Option<&ReadPatchFn>;
    fn bmc(&self) -> &B;

    async fn update_with_patch(&self, update: &V) -> Result<T, Error<B>> {
        if let Some(patch_fn) = self.patch() {
            Updator {
                id: self.entity_ref().id(),
                etag: self.entity_ref().etag(),
            }
            .update(self.bmc(), update, patch_fn.as_ref())
            .await
        } else {
            self.entity_ref()
                .update(self.bmc(), update)
                .await
                .map_err(Error::Bmc)
        }
    }
}

/// Support payload patching.
///
/// This struct supports deserialization from any JSON payload and
/// provides a method to apply a patch and then deserialize to the
/// target type.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Payload(JsonValue);

impl Payload {
    #[cfg(feature = "patch-payload-get")]
    pub(crate) async fn get<T, B, F>(
        bmc: &B,
        nav: &NavProperty<T>,
        f: F,
    ) -> Result<Arc<T>, Error<B>>
    where
        T: EntityTypeRef + for<'a> Deserialize<'a> + Send + Sync + 'static,
        B: Bmc,
        F: FnOnce(JsonValue) -> JsonValue,
    {
        match nav {
            NavProperty::Expanded(_) => nav.get(bmc).await.map_err(Error::Bmc),
            NavProperty::Reference(_) => {
                let getter = NavProperty::<Getter>::new_reference(nav.id().clone());
                let v = getter.get(bmc).await.map_err(Error::Bmc)?;
                v.payload.to_target(f).map(Arc::new)
            }
        }
    }

    #[cfg(feature = "patch-payload-expand")]
    pub(crate) async fn expand_property<T, B, F>(
        bmc: &NvBmc<B>,
        nav: &NavProperty<T>,
        f: F,
    ) -> Result<Arc<T>, Error<B>>
    where
        T: EntityTypeRef + for<'a> Deserialize<'a> + Send + Sync + 'static,
        B: Bmc,
        F: FnOnce(JsonValue) -> JsonValue,
    {
        match nav {
            NavProperty::Expanded(_) => nav.get(bmc.as_ref()).await.map_err(Error::Bmc),
            NavProperty::Reference(_) => {
                let getter = NavProperty::<Getter>::new_reference(nav.id().clone());
                let v = bmc.expand_property(&getter).await?;
                v.payload.to_target(f).map(Arc::new)
            }
        }
    }

    /// Apply function `f` to the payload and then try to deserialize to the
    /// target type.
    pub(crate) fn to_target<T, B, F>(&self, f: F) -> Result<T, Error<B>>
    where
        T: for<'de> Deserialize<'de>,
        B: Bmc,
        F: FnOnce(JsonValue) -> JsonValue,
    {
        serde_json::from_value(f(self.0.clone())).map_err(Error::Json)
    }
}

struct Getter {
    id: ODataId,
    payload: Payload,
}

impl EntityTypeRef for Getter {
    fn id(&self) -> &ODataId {
        &self.id
    }
    fn etag(&self) -> Option<&ODataETag> {
        None
    }
}

impl Expandable for Getter {}

impl<'de> Deserialize<'de> for Getter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            id: String::new().into(),
            payload: Payload::deserialize(deserializer)?,
        })
    }
}

#[cfg(feature = "patch-payload-update")]
struct Updator<'a> {
    id: &'a ODataId,
    etag: Option<&'a ODataETag>,
}

#[cfg(feature = "patch-payload-update")]
impl EntityTypeRef for Updator<'_> {
    fn id(&self) -> &ODataId {
        self.id
    }
    fn etag(&self) -> Option<&ODataETag> {
        self.etag
    }
}

#[cfg(feature = "patch-payload-update")]
impl Updator<'_> {
    async fn update<B, U, T, F>(&self, bmc: &B, update: &U, patch_fn: F) -> Result<T, Error<B>>
    where
        B: Bmc,
        T: EntityTypeRef + for<'de> Deserialize<'de> + Sync + Send,
        U: Serialize + Send + Sync,
        F: Fn(JsonValue) -> JsonValue + Sync + Send,
    {
        bmc.update::<U, Payload>(self.id(), self.etag(), update)
            .await
            .map_err(Error::Bmc)?
            .to_target(patch_fn)
    }
}
