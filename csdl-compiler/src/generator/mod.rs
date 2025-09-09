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

use crate::edmx::{IsBound, IsNullable, PropertyName, SchemaNamespace, TypeName};
use crate::odata::annotations::{Description, LongDescription};
use alloc::rc::Rc;
use tagged_types::TaggedType;

pub type PropertyUnits = TaggedType<String, PropertyUnitsTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq)]
#[transparent(Debug, Display, Deserialize)]
#[capability(inner_access)]
pub enum PropertyUnitsTag {}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug)]
pub enum ResourceTypeKind {
    String,
    Boolean,
    Decimal,
    Int32,
    Int64,

    Collection(Rc<ResourceTypeKind>),

    ComplexType(Rc<ComplexTypeData>),
    EnumType(Vec<EnumMember>),
}


#[derive(Debug)]
pub struct ResourceTypeName {
    pub namespace: SchemaNamespace,
    pub name: TypeName,
}

#[derive(Debug)]
pub struct ResourceType {
    pub name: ResourceTypeName,
    pub metadata: ResourceMetadata,
    pub kind: ResourceTypeKind,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RedfishUri {
    pub segments: Vec<UriSegment>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UriSegment {
    Static(String),
    Parameter(String),
}

#[derive(Debug)]
pub struct RedfishResource {
    pub base_type: ResourceType,
    pub uris: Vec<RedfishUri>,
    pub capabilities: Capabilities,
    pub actions: Vec<ResourceAction>,
    pub properties: Vec<ResourceProperty>,
}

#[derive(Debug)]
pub struct ResourceMetadata {
    pub description: Description,
    pub long_description: Option<LongDescription>,
}

#[derive(Debug)]
pub enum ResourceProperty {
    Property(PropertyData),
    NavigationProperty(NavigationPropertyData),
}

#[derive(Debug)]
pub struct PropertyData {
    pub name: PropertyName,
    pub nullable: Option<IsNullable>,
    pub permissions: Permission,
    pub units: Option<PropertyUnits>,
    pub constraints: Option<Constraints>,
}

#[derive(Debug)]
pub struct NavigationPropertyData {
    pub name: PropertyName,
    pub nullable: Option<IsNullable>,
    pub permissions: Permission,
    pub auto_expand: bool,
    pub excerpt_copy: Option<String>,
}

#[derive(Debug)]
pub struct ComplexTypeData {
    pub base_type: Option<ResourceType>,
    pub properties: Vec<ResourceProperty>,
    pub additional_properties: bool,
}

#[derive(Debug)]
pub struct EnumMember {
    pub name: String,
    pub description: Option<Description>,
}

#[derive(Debug)]
pub struct ResourceAction {
    pub is_bound: IsBound,
    pub metadata: ResourceMetadata,
    pub properteies: Vec<ResourceProperty>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Permission {
    Read,
    Write,
    ReadWrite,
    None,
}

#[derive(Debug)]
pub struct Constraints {
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub pattern: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    pub insertable: CapabilityInfo,
    pub updatable: CapabilityInfo,
    pub deletable: CapabilityInfo,
}

#[derive(Debug, Clone)]
pub struct CapabilityInfo {
    pub enabled: bool,
    pub description: Option<Description>,
}
