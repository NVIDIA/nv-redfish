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

/// Type for desrialization of Action.
pub mod action;
/// Type for `@odata.id` identifier.
pub mod bmc;
/// Adaptive cache implementation using Clock-CAR algorithm
pub mod cache;
/// Custom desrialization.
pub mod deserialize;
/// `Edm.DateTimeOffset` type.
pub mod edm_date_time_offset;
/// Type that represents `Edm.Duration`.
pub mod edm_duration;
/// HTTP client abstractions and Redfish expand query support
pub mod http;
/// Type for navigation property.
pub mod nav_property;
/// Type for `@odata.id` identifier.
pub mod odata;

use crate::http::ExpandQuery;
use serde::{Deserialize, Deserializer, Serialize};
use std::{future::Future, sync::Arc};

#[doc(inline)]
pub use action::Action;
#[doc(inline)]
pub use action::ActionError;
#[doc(inline)]
pub use bmc::Bmc;
#[doc(inline)]
pub use deserialize::de_optional_nullable;
#[doc(inline)]
pub use deserialize::de_required_nullable;
#[doc(inline)]
pub use edm_date_time_offset::EdmDateTimeOffset;
#[doc(inline)]
pub use edm_duration::EdmDuration;
#[doc(inline)]
pub use nav_property::NavProperty;
#[doc(inline)]
pub use nav_property::Reference;
#[doc(inline)]
pub use odata::ODataETag;
#[doc(inline)]
pub use odata::ODataId;
#[doc(inline)]
pub use serde_json::Value as AdditionalProperties;
#[doc(inline)]
pub use uuid::Uuid as EdmGuid;

/// Entity type reference trait that is implemented by CSDL compiler
/// for all generated entity types and for all `NavProperty<T>` where
/// T struct for entity type.
pub trait EntityTypeRef {
    /// Value of `@odata.id` field of the Entity.
    fn id(&self) -> &ODataId;

    /// Value of `@odata.etag` field of the Entity.
    fn etag(&self) -> Option<&ODataETag>;

    /// Update entity using `update` as payload.
    fn refresh<B: Bmc>(&self, bmc: &B) -> impl Future<Output = Result<Arc<Self>, B::Error>> + Send
    where
        Self: Sync + Send + 'static + Sized + for<'de> Deserialize<'de>,
    {
        bmc.get::<Self>(self.id())
    }
}

pub trait Expandable: EntityTypeRef + Sized + for<'a> Deserialize<'a> {
    /// Expand entity type.
    fn expand<B: Bmc>(
        &self,
        bmc: &B,
        query: ExpandQuery,
    ) -> impl Future<Output = Result<Arc<Self>, B::Error>> + Send {
        bmc.expand::<Self>(self.id(), query)
    }
}

/// Empty struct, denotes unit return type, used for Redfish responses which are expected to
/// not return any json data
#[derive(Debug)]
pub struct Empty {}

impl<'de> Deserialize<'de> for Empty {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Empty {})
    }
}

/// This trait is assigned to entity types that are marked as
/// updatable in CSDL specification.
pub trait Creatable<V: Sync + Send + Serialize, R: Sync + Send + Sized + for<'de> Deserialize<'de>>:
    EntityTypeRef + Sized
{
    /// Create entity type `create` as payload.
    fn create<B: Bmc>(
        &self,
        bmc: &B,
        create: &V,
    ) -> impl Future<Output = Result<R, B::Error>> + Send {
        bmc.create::<V, R>(self.id(), create)
    }
}

/// This trait is assigned to entity types that are marked as
/// updatable in CSDL specification.
pub trait Updatable<V: Sync + Send + Serialize>: EntityTypeRef + Sized
where
    Self: Sync + Send + Sized + for<'de> Deserialize<'de>,
{
    /// Update entity using `update` as payload.
    fn update<B: Bmc>(
        &self,
        bmc: &B,
        update: &V,
    ) -> impl Future<Output = Result<Self, B::Error>> + Send {
        bmc.update::<V, Self>(self.id(), update)
    }
}

/// This trait is assigned to entity types that are marked as
/// deletable in CSDL specification.
pub trait Deletable: EntityTypeRef + Sized {
    /// Delete current entity.
    fn delete<B: Bmc>(&self, bmc: &B) -> impl Future<Output = Result<Empty, B::Error>> + Send {
        bmc.delete(self.id())
    }
}
