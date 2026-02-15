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
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::resource::ItemOrCollection;
use crate::schema::redfish::resource::Oem;
use crate::schema::redfish::resource::ResourceCollection;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::Deserialize;
use std::sync::Arc;

#[cfg(feature = "patch-collection-create")]
use nv_redfish_core::Creatable;
#[cfg(feature = "patch-collection-create")]
use serde::Serialize;

/// Trait that allows patching collection member data before it is
/// deserialized to the member data structure. This is required when a
/// BMC implementation produces payloads that are not aligned with the
/// CSDL schema.
///
/// Example of usage is in `AccountCollection` implementation.
pub trait CollectionWithPatch<T, M, B>
where
    T: EntityTypeRef + Expandable + Send + Sync + 'static,
    M: EntityTypeRef + Send + Sync + for<'de> Deserialize<'de>,
    B: Bmc,
{
    fn convert_patched(base: ResourceCollection, members: Vec<NavProperty<M>>) -> T;

    async fn expand_collection(
        bmc: &NvBmc<B>,
        nav: &NavProperty<T>,
        patch_fn: Option<&ReadPatchFn>,
    ) -> Result<Arc<T>, Error<B>> {
        if let Some(patch_fn) = patch_fn {
            // Patches are not free so we keep separate branch for
            // patched collections only having this cost on systems
            // that requires to pay the price.
            let patched_collection_ref = NavProperty::<Collection>::new_reference(nav.id().clone());
            let collection = bmc.expand_property(&patched_collection_ref).await?;
            let members = collection.members(&patch_fn.as_ref())?;
            Ok(Arc::new(Self::convert_patched(collection.base(), members)))
        } else {
            bmc.expand_property(nav).await
        }
    }
}

/// Trait that allows creating a collection member and patching the
/// response before it is deserialized to the member data structure.
///
/// Example of usage is in `AccountCollection` implementation.
#[cfg(feature = "patch-collection-create")]
pub trait CreateWithPatch<T, M, C, B>
where
    T: EntityTypeRef + Creatable<C, M> + Sync + Send,
    C: Serialize + Sync + Send,
    M: for<'de> Deserialize<'de> + Sync + Send,
    B: Bmc,
{
    fn entity_ref(&self) -> &T;
    fn patch(&self) -> Option<&ReadPatchFn>;
    fn bmc(&self) -> &B;

    async fn create_with_patch(&self, create: &C) -> Result<M, Error<B>> {
        if let Some(patch_fn) = &self.patch() {
            Collection::create(self.entity_ref(), self.bmc(), create, patch_fn.as_ref()).await
        } else {
            self.entity_ref()
                .create(self.bmc(), create)
                .await
                .map_err(Error::Bmc)
        }
    }
}

/// Collection of entity types that can apply patches to its members on read.
///
/// In some situations, a BMC implementation may miss fields that are
/// marked as required but have reasonable defaults. This collection
/// can be used to deserialize the collection and then restore the
/// original shape by patching member payloads.
#[derive(Deserialize)]
struct Collection {
    #[serde(flatten)]
    base: ResourceCollection,
    #[serde(rename = "Members")]
    members: Vec<Payload>,
}

impl Collection {
    #[cfg(feature = "patch-collection-create")]
    async fn create<T, F, C, B, V>(orig: &T, bmc: &B, create: &C, f: F) -> Result<V, Error<B>>
    where
        T: EntityTypeRef + Sync + Send,
        V: for<'de> Deserialize<'de>,
        B: Bmc,
        C: Serialize + Sync + Send,
        F: FnOnce(JsonValue) -> JsonValue + Sync + Send,
    {
        Creator { id: orig.id() }
            .create(bmc, create)
            .await
            .map_err(Error::Bmc)?
            .to_target(f)
    }

    fn base(&self) -> ResourceCollection {
        ResourceCollection {
            base: ItemOrCollection {
                odata_id: self.base.base.odata_id.clone(),
                odata_etag: self.base.base.odata_etag.clone(),
                odata_type: self.base.base.odata_type.clone(),
                // Don't support `@Redfish.Settings /
                // @Redfish.SettingsApplyTime` for patched
                // collection...
                redfish_settings: None,
                redfish_settings_apply_type: None,
            },
            description: self.base.description.clone(),
            name: self.base.name.clone(),
            oem: self.base.oem.as_ref().map(|oem| Oem {
                additional_properties: oem.additional_properties.clone(),
            }),
        }
    }

    fn members<T, F, B>(&self, f: &F) -> Result<Vec<NavProperty<T>>, Error<B>>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de>,
        F: Fn(JsonValue) -> JsonValue,
        B: Bmc,
    {
        self.members
            .iter()
            .map(|v| v.to_target(f))
            .collect::<Result<Vec<_>, _>>()
    }
}

impl EntityTypeRef for Collection {
    fn id(&self) -> &ODataId {
        self.base.id()
    }
    fn etag(&self) -> Option<&ODataETag> {
        self.base.etag()
    }
}

impl Expandable for Collection {}

// Helper struct that enables creating a new member of the collection
// and applying a patch to the payload before creation.
#[cfg(feature = "patch-collection-create")]
struct Creator<'a> {
    id: &'a ODataId,
}

#[cfg(feature = "patch-collection-create")]
impl EntityTypeRef for Creator<'_> {
    fn id(&self) -> &ODataId {
        self.id
    }
    fn etag(&self) -> Option<&ODataETag> {
        None
    }
}

#[cfg(feature = "patch-collection-create")]
impl<V: Serialize + Send + Sync> Creatable<V, Payload> for Creator<'_> {}
