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

//! Redfish-specific attributes used during code generation.

use crate::redfish::annotations::RedfishAnnotations;
use crate::redfish::DynamicProperties;
use crate::redfish::Excerpt;
use crate::redfish::ExcerptCopy;
use crate::IsExcerptCopyOnly;
use crate::IsRequired;
use crate::IsRequiredOnCreate;

/// Redfish property attributes attached to compiled entities.
#[derive(Debug)]
pub struct RedfishProperty {
    /// Whether the property is required.
    pub is_required: IsRequired,
    /// Whether the property is required on create.
    pub is_required_on_create: IsRequiredOnCreate,
    /// Whether the property is only appear in excerpt copies of the resource.
    pub is_excerpt_only: IsExcerptCopyOnly,
    /// Defines which excerpt view property belongs to.
    pub excerpt: Option<Excerpt>,
    /// Property is excerpt copy of the resource.
    pub excerpt_copy: Option<ExcerptCopy>,
}

impl RedfishProperty {
    /// Create a new instance from an object that provides Redfish
    /// property annotations.
    pub fn new(src: &impl RedfishAnnotations) -> Self {
        Self {
            is_required: src.is_required(),
            is_required_on_create: src.is_required_on_create(),
            is_excerpt_only: src.is_excerpt_only(),
            excerpt: src.excerpt(),
            excerpt_copy: src.excerpt_copy(),
        }
    }
}

/// Redfish attributes attached to types.
#[derive(Debug)]
pub struct Redfish<'a> {
    /// Dynamic properties defined for the type.
    pub dynamic_properties: Option<DynamicProperties<'a>>,
}

impl<'a> Redfish<'a> {
    /// Create a new instance from an object that provides Redfish
    /// property annotations.
    pub fn new(src: &'a impl RedfishAnnotations) -> Self {
        Self {
            dynamic_properties: src.dynamic_properties(),
        }
    }
}
