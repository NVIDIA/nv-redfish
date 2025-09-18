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

use crate::compiler::CompiledNavProperty;
use crate::compiler::CompiledProperty;
use crate::compiler::CompiledOData;
use crate::compiler::MapType;
use crate::compiler::QualifiedName;
use crate::compiler::redfish::RedfishProperty;
use crate::edmx::PropertyName;
use crate::edmx::attribute_values::TypeName;

/// Combination of all compiled properties and navigation properties.
#[derive(Default, Debug)]
pub struct CompiledProperties<'a> {
    pub properties: Vec<CompiledProperty<'a>>,
    pub nav_properties: Vec<CompiledNavProperty<'a>>,
}

impl CompiledProperties<'_> {
    /// Join properties in reverse order. This function is useful when
    /// compiler have list of current object and all parents and it
    /// needs all properties in order from parent to child.
    #[must_use]
    pub fn rev_join(src: Vec<Self>) -> Self {
        let (properties, nav_properties): (Vec<_>, Vec<_>) = src
            .into_iter()
            .map(|v| (v.properties, v.nav_properties))
            .unzip();
        Self {
            properties: properties.into_iter().rev().flatten().collect(),
            nav_properties: nav_properties.into_iter().rev().flatten().collect(),
        }
    }

    /// No properties defined.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.nav_properties.is_empty()
    }
}

#[derive(Debug)]
pub enum CompiledPropertyType<'a> {
    One(QualifiedName<'a>),
    CollectionOf(QualifiedName<'a>),
}

impl<'a> CompiledPropertyType<'a> {
    #[must_use]
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        match self {
            Self::One(v) => Self::One(f(v)),
            Self::CollectionOf(v) => Self::CollectionOf(f(v)),
        }
    }
}

impl<'a> From<&'a TypeName> for CompiledPropertyType<'a> {
    fn from(v: &'a TypeName) -> Self {
        match v {
            TypeName::One(v) => Self::One(v.into()),
            TypeName::CollectionOf(v) => Self::CollectionOf(v.into()),
        }
    }
}

#[derive(Debug)]
pub struct CompiledProperty<'a> {
    pub name: &'a PropertyName,
    pub ptype: CompiledPropertyType<'a>,
    pub odata: CompiledOData<'a>,
    pub redfish: RedfishProperty,
}

impl<'a> MapType<'a> for CompiledProperty<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = self.ptype.map(f);
        self
    }
}

#[derive(Debug)]
pub struct CompiledNavProperty<'a> {
    pub name: &'a PropertyName,
    pub ptype: CompiledPropertyType<'a>,
    pub odata: CompiledOData<'a>,
    pub redfish: RedfishProperty,
}

impl<'a> MapType<'a> for CompiledNavProperty<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = self.ptype.map(f);
        self
    }
}
