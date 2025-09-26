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
/// HTTP client abstractions and Redfish expand query support
pub mod http;
/// Type for navigation property.
pub mod nav_property;
/// Type for `@odata.id` identifier.
pub mod odata;

use crate::http::ExpandQuery;
use serde::{Deserialize, Deserializer};
use std::{future::Future, sync::Arc};

#[doc(inline)]
pub use action::Action;
#[doc(inline)]
pub use action::ActionError;
#[doc(inline)]
pub use bmc::Bmc;
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

/// Entity type trait that is implemented by CSDL compiler for all
/// generated entity types.
pub trait EntityType {
    /// Value of `@odata.id` field of the Entity.
    fn id(&self) -> &ODataId;

    /// Value of `@odata.etag` field of the Entity.
    fn etag(&self) -> &Option<ODataETag>;
}

pub trait Expandable: EntityType + Sized + for<'a> Deserialize<'a> {
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
