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

/// 3.1 Element edmx:Edmx
pub mod edmx_root;

/// 3.2 Element edmx:DataServicse
pub mod data_services;

/// 3.3 Element edmx:Reference
pub mod reference;

/// 3.4 Element edmx:Include
pub mod include;

/// 3.5 Element edmx:IncludeAnnotations
pub mod include_annotations;

/// 5 Schema
pub mod schema;

use quick_xml::DeError;
use serde::Deserialize;

/// EDMX compilation errors.
#[derive(Debug)]
pub enum ValidateError {
    /// XML deserialization error.
    XmlDeserialize(DeError),
    /// Invalid number of `DataServices`.
    WrongDataServicesNumber,
}

/// Rexport of Edmx type to root.
pub type Edmx = edmx_root::Edmx;

#[derive(Debug, Deserialize)]
pub struct EntityType {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@BaseType")]
    pub base_type: Option<String>,
    #[serde(rename = "@Abstract")]
    pub r#abstract: Option<bool>,
    #[serde(rename = "@OpenType")]
    pub open_type: Option<bool>,
    #[serde(rename = "@HasStream")]
    pub has_stream: Option<bool>,
    #[serde(rename = "Key")]
    pub key: Option<Key>,
    #[serde(rename = "$value", default)]
    pub items: Vec<EntityTypeItem>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub enum EntityTypeItem {
    Property(Property),
    NavigationProperty(NavigationProperty),
}

#[derive(Debug, Deserialize)]
pub struct Key {
    #[serde(rename = "PropertyRef", default)]
    pub property_refs: Vec<PropertyRef>,
}

#[derive(Debug, Deserialize)]
pub struct PropertyRef {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Alias")]
    pub alias: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Property {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Type")]
    pub r#type: String,
    #[serde(rename = "@Nullable")]
    pub nullable: Option<bool>,
    #[serde(rename = "@MaxLength")]
    pub max_length: Option<String>,
    #[serde(rename = "@Precision")]
    pub precision: Option<i32>,
    #[serde(rename = "@Scale")]
    pub scale: Option<String>, // "variable" or number
    #[serde(rename = "@Unicode")]
    pub unicode: Option<bool>,
    #[serde(rename = "@DefaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NavigationProperty {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Type")]
    pub r#type: String,
    #[serde(rename = "@Nullable")]
    pub nullable: Option<bool>,
    #[serde(rename = "@Partner")]
    pub partner: Option<String>,
    #[serde(rename = "@ContainsTarget")]
    pub contains_target: Option<bool>,
    #[serde(rename = "ReferentialConstraint", default)]
    pub referential_constraints: Vec<ReferentialConstraint>,
    #[serde(rename = "OnDelete")]
    pub on_delete: Option<OnDelete>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct ReferentialConstraint {
    #[serde(rename = "@Property")]
    pub property: String,
    #[serde(rename = "@ReferencedProperty")]
    pub referenced_property: String,
}

#[derive(Debug, Deserialize)]
pub struct OnDelete {
    #[serde(rename = "@Action")]
    pub action: String, // e.g., "Cascade", "None"
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ComplexType {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@BaseType")]
    pub base_type: Option<String>,
    #[serde(rename = "@Abstract")]
    pub r#abstract: Option<bool>,
    #[serde(rename = "@OpenType")]
    pub open_type: Option<bool>,
    #[serde(rename = "Property", default)]
    pub properties: Vec<Property>,
    #[serde(rename = "NavigationProperty", default)]
    pub navigation_properties: Vec<NavigationProperty>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EnumType {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@UnderlyingType")]
    pub underlying_type: Option<String>,
    #[serde(rename = "@IsFlags")]
    pub is_flags: Option<bool>,
    #[serde(rename = "Member", default)]
    pub members: Vec<EnumMember>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct EnumMember {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Value")]
    pub value: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct TypeDefinition {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@UnderlyingType")]
    pub underlying_type: String,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct EntityContainer {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "EntitySet", default)]
    pub entity_sets: Vec<EntitySet>,
    #[serde(rename = "Singleton", default)]
    pub singletons: Vec<Singleton>,
    #[serde(rename = "ActionImport", default)]
    pub action_imports: Vec<ActionImport>,
    #[serde(rename = "FunctionImport", default)]
    pub function_imports: Vec<FunctionImport>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct EntitySet {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@EntityType")]
    pub entity_type: String,
    #[serde(rename = "NavigationPropertyBinding", default)]
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct NavigationPropertyBinding {
    #[serde(rename = "@Path")]
    pub path: String,
    #[serde(rename = "@Target")]
    pub target: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Singleton {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Type")]
    pub r#type: String,
    #[serde(rename = "NavigationPropertyBinding", default)]
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct ActionImport {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Action")]
    pub action: String,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct FunctionImport {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Function")]
    pub function: String,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct Annotation {
    #[serde(rename = "@Term")]
    pub term: String,
    #[serde(rename = "@String")]
    pub string: Option<String>,
    #[serde(rename = "@Bool")]
    pub bool_value: Option<bool>,
    #[serde(rename = "@Int")]
    pub int_value: Option<i64>,
    #[serde(rename = "@EnumMember")]
    pub enum_member: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Annotations {
    #[serde(rename = "@Target")]
    pub target: String,
    #[serde(rename = "@Qualifier")]
    pub qualifier: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct Term {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Type")]
    pub ttype: Option<String>,
    #[serde(rename = "@AppliesTo")]
    pub applies_to: Option<String>,
    #[serde(rename = "@DefaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}
