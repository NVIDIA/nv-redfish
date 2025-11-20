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

//! Redfish resource

use crate::ResourceSchema;
use crate::ResourceStatusSchema;
use std::convert::identity;
use tagged_types::TaggedType;

#[doc(inline)]
pub use crate::schema::redfish::resource::Health;
#[doc(inline)]
pub use crate::schema::redfish::resource::State;

#[doc(inline)]
#[cfg(feature = "computer-systems")]
pub use crate::schema::redfish::resource::PowerState;

/// Redfish resource identifier.
pub type ResourceId = TaggedType<String, ResourceIdTag>;
/// Reference to Redfish resource identifier.
pub type ResourceIdRef<'a> = TaggedType<&'a String, ResourceIdTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ResourceIdTag {}

/// Redfish resource name.
pub type ResourceName = TaggedType<String, ResourceNameTag>;
/// Reference to Redfish resource name.
pub type ResourceNameRef<'a> = TaggedType<&'a String, ResourceNameTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ResourceNameTag {}

/// Redfish resource description.
pub type ResourceDescription = TaggedType<String, ResourceDescriptionTag>;
/// Reference to Redfish resource description.
pub type ResourceDescriptionRef<'a> = TaggedType<&'a String, ResourceDescriptionTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone)]
#[transparent(Debug, Display, FromStr, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum ResourceDescriptionTag {}

/// Represents Redfish Resource base type.
pub trait Resource {
    /// Required function. Must be implemented for Redfish resources.
    fn resource_ref(&self) -> &ResourceSchema;

    /// Identifier of the resource.
    fn id(&self) -> ResourceIdRef<'_> {
        ResourceIdRef::new(&self.resource_ref().id)
    }

    /// Name of the resource.
    fn name(&self) -> ResourceNameRef<'_> {
        ResourceNameRef::new(&self.resource_ref().name)
    }

    /// Description of the resource.
    fn description(&self) -> Option<ResourceDescriptionRef<'_>> {
        self.resource_ref()
            .description
            .as_ref()
            .and_then(|v| v.as_ref())
            .map(ResourceDescriptionRef::new)
    }
}

/// The status and health of a resource and its children.
#[derive(Clone, Debug)]
pub struct Status {
    /// The state of the resource.
    pub state: Option<State>,
    /// The health state of this resource in the absence of its dependent resources.
    pub health: Option<Health>,
    /// The overall health state from the view of this resource.
    pub health_rollup: Option<Health>,
}

/// Represents Redfish resource that provides it's status.
pub trait ResourceProvidesStatus {
    /// Required function. Must be implemented for Redfish resources
    /// that provides resource status.
    fn resource_status_ref(&self) -> Option<&ResourceStatusSchema>;

    /// Status of the resource if it is provided.
    fn status(&self) -> Option<Status> {
        self.resource_status_ref().map(|status| Status {
            state: status.state.and_then(identity),
            health: status.health.and_then(identity),
            health_rollup: status.health_rollup.and_then(identity),
        })
    }
}
