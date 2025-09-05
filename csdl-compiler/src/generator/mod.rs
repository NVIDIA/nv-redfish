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

use crate::odata::annotations::{Description, LongDescription, ODataAnnotations};
use alloc::rc::Rc;
use tagged_types::TaggedType;

pub mod converter;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug)]
pub struct VersionedField<T> {
    pub field: T,
    pub introduced_in: Version,
    pub deprecated_in: Option<Version>,
}

#[derive(Debug)]
pub struct RedfishResource {
    pub metadata: ItemMetadata,
    pub uris: Vec<String>,
    pub items: Vec<VersionedField<ResourceItem>>,
    pub capabilities: Capabilities,
}

#[derive(Debug)]
pub struct ItemMetadata {
    pub name: String,
    pub description: Description,
    pub long_description: Option<LongDescription>,
}

impl ItemMetadata {
    pub fn new(name: String, odata: &impl ODataAnnotations) -> Self {
        Self {
            name,
            description: odata.odata_description_or_default(),
            long_description: odata.odata_long_description().map(TaggedType::cloned),
        }
    }
}

#[derive(Debug)]
pub enum ResourceItem {
    Property(PropertyData),
    NavigationProperty(NavigationPropertyData),
    Action(ActionData),
}

#[derive(Debug)]
pub struct PropertyData {
    pub metadata: ItemMetadata,
    pub property_type: PropertyType,
    pub nullable: Option<bool>,
    pub permissions: Permission,
    pub units: Option<String>,
    pub constraints: Option<Constraints>,
}

#[derive(Debug)]
pub struct NavigationPropertyData {
    pub metadata: ItemMetadata,
    pub target_type: ResourceReference,
    pub is_collection: bool,
    pub nullable: Option<bool>,
    pub permissions: Permission,
    pub auto_expand: bool,
    pub excerpt_copy: Option<String>,
}

#[derive(Debug)]
pub struct ComplexTypeData {
    pub metadata: ItemMetadata,
    pub base_type: Option<ResourceReference>,
    pub properties: Vec<PropertyData>,
    pub navigation_properties: Vec<NavigationPropertyData>,
    pub additional_properties: bool,
}

#[derive(Debug)]
pub struct EnumData {
    pub metadata: ItemMetadata,
    pub members: Vec<EnumMember>,
}

#[derive(Debug)]
pub struct EnumMember {
    pub name: String,
    pub description: Option<Description>,
}

#[derive(Debug)]
pub struct ActionData {
    pub metadata: ItemMetadata,
    pub is_bound: bool,
    pub parameters: Vec<ActionParameter>,
}

#[derive(Debug)]
pub struct ActionParameter {
    pub metadata: ItemMetadata,
    pub parameter_type: PropertyType,
    pub nullable: Option<bool>,
}

#[derive(Debug, Clone)]
pub enum PropertyType {
    // Edm types
    String,
    Boolean,
    Decimal,
    Int32,
    Int64,

    Collection(Rc<PropertyType>),

    Reference(ResourceReference),
}

#[derive(Debug)]
pub enum ReferencedType {
    ComplexType(ComplexTypeData),
    Enum(EnumData),
}

#[derive(Debug, Clone)]
pub enum ResourceReference {
    LocalType(Rc<VersionedField<ReferencedType>>),
    External(Rc<RedfishResource>),

    // TODO: This is temporary, just to be able to test without compiling all references for all external resources
    TypeName(String),
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
    pub insertable: Option<CapabilityInfo>,
    pub updatable: Option<CapabilityInfo>,
    pub deletable: Option<CapabilityInfo>,
}

#[derive(Debug, Clone)]
pub struct CapabilityInfo {
    pub enabled: bool,
    pub description: Option<String>,
}
