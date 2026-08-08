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

use crate::compiler::MapType;
use crate::compiler::NavPropertyType;
use crate::compiler::OData;
use crate::compiler::PropertyType;
use crate::compiler::QualifiedName;
use crate::edmx::ParameterName;
use crate::redfish::Deprecation;
use crate::IsNullable;
use crate::IsRequired;

/// Compiled action parameter.
#[derive(Debug, Clone)]
pub struct Parameter<'a> {
    /// Name of the parameter.
    pub name: &'a ParameterName,
    /// Parameter type: either an entity reference or a specific type.
    pub ptype: ParameterType<'a>,
    /// Whether the parameter is nullable.
    pub nullable: IsNullable,
    /// Whether the parameter is required.
    pub required: IsRequired,
    /// `OData` annotations for the parameter.
    pub odata: OData<'a>,
    /// Deprecation revision, if the schema marks the parameter
    /// deprecated.
    pub deprecation: Option<Box<Deprecation>>,
}

/// Parameter type. Reuses `CompiledPropertyType`; this may not be an
/// exact match and could evolve in the future.
#[derive(Debug, Clone, Copy)]
pub enum ParameterType<'a> {
    /// Entity parameter (navigation target).
    Entity(NavPropertyType<'a>),
    /// Non-entity parameter (complex/simple type).
    Type(PropertyType<'a>),
}

impl<'a> ParameterType<'a> {
    fn map<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        match self {
            Self::Entity(v) => Self::Entity(v.map(f)),
            Self::Type(v) => Self::Type(v.map(|(typeclass, ptype)| (typeclass, f(ptype)))),
        }
    }
}

impl<'a> MapType<'a> for Parameter<'a> {
    fn map_type<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        Self {
            name: self.name,
            ptype: self.ptype.map(f),
            nullable: self.nullable,
            required: self.required,
            odata: self.odata,
            deprecation: self.deprecation,
        }
    }
}
