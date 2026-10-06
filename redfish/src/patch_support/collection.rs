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

use crate::patch_support::FilterFn;
use crate::patch_support::JsonValue;
use crate::patch_support::Payload;
use crate::schema::resource::ItemOrCollection;
use crate::schema::resource::Oem;
use crate::schema::resource::ResourceCollection;
use crate::schema::SettingsAnnotations;
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

/// Trait for collections whose members a platform quirk shapes before they
/// are deserialized: members to leave out, or `Members` sent as `null`.
/// Member documents themselves are already repaired by the compatibility
/// layer.
///
/// Example of usage is in `ManagerCollection` implementation.
pub trait CollectionWithPatch<T, M, B>
where
    T: Expandable + 'static,
    M: EntityTypeRef + for<'de> Deserialize<'de>,
    B: Bmc,
{
    fn convert_patched(base: ResourceCollection, members: Vec<NavProperty<M>>) -> T;

    async fn expand_collection(
        bmc: &NvBmc<B>,
        nav: &NavProperty<T>,
        filter_fn: Option<&FilterFn>,
    ) -> Result<Arc<T>, Error<B>> {
        if filter_fn.is_some() || bmc.quirks.bug_nullable_members() {
            // Reading members as payloads is not free, so only the
            // platforms that need it pay for it.
            let patched_collection_ref = NavProperty::<Collection>::new_reference(nav.id().clone());
            let collection = bmc.expand_property(&patched_collection_ref).await?;
            let filter_fn = filter_fn.map(AsRef::as_ref);
            let members = collection.members(filter_fn)?;
            Ok(Arc::new(Self::convert_patched(collection.base(), members)))
        } else {
            bmc.expand_property(nav).await
        }
    }
}

/// A collection read with its members as payloads.
#[derive(Deserialize)]
struct Collection {
    #[serde(flatten)]
    base: ResourceCollection,
    #[serde(rename = "Members")]
    members: Option<Vec<Payload>>,
}

impl Collection {
    fn base(&self) -> ResourceCollection {
        ResourceCollection {
            base: ItemOrCollection {
                odata_id: self.base.base.odata_id.clone(),
                odata_etag: self.base.base.odata_etag.clone(),
                // Don't support `@Redfish.Settings /
                // @Redfish.SettingsApplyTime` for patched
                // collection...
                settings_annotations: SettingsAnnotations::default(),
            },
            odata_type: self.base.odata_type.clone(),
            description: self.base.description.clone(),
            name: self.base.name.clone(),
            oem: self.base.oem.as_ref().map(|oem| Oem {
                additional_properties: oem.additional_properties.clone(),
            }),
        }
    }

    fn members<T, FF, B>(&self, filter_fn: Option<&FF>) -> Result<Vec<NavProperty<T>>, Error<B>>
    where
        T: EntityTypeRef + for<'de> Deserialize<'de>,
        FF: Fn(&JsonValue) -> bool + ?Sized,
        B: Bmc,
    {
        self.members
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|v| filter_fn.is_none_or(|ff| v.filter(ff)))
            .map(Payload::parse)
            .collect::<Result<Vec<_>, _>>()
    }
}

impl EntityTypeRef for Collection {
    fn odata_id(&self) -> &ODataId {
        self.base.odata_id()
    }
    fn etag(&self) -> Option<&ODataETag> {
        self.base.etag()
    }
}

impl Expandable for Collection {}
