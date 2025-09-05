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

/// 6 Structural Property / 7 Navigation Property
pub mod property;

/// 8 Entity Type
pub mod entity_type;

/// 9 Complex Type
pub mod complex_type;

use quick_xml::DeError;
use serde::Deserialize;

pub type TypeName = String;
pub type SchemaNamespace = String;
pub type PropertyName = String;

/// EDMX compilation errors.
#[derive(Debug)]
pub enum ValidateError {
    /// XML deserialization error.
    XmlDeserialize(DeError),
    /// Invalid number of `DataServices`.
    WrongDataServicesNumber,
    /// In the `EntityType` too many keys.
    TooManyKeys,
    /// Schema validation error.
    Schema(SchemaNamespace, Box<ValidateError>),
    /// `ComplexType` validation error.
    ComplexType(TypeName, Box<ValidateError>),
    /// `EntityType` validation error.
    EntityType(TypeName, Box<ValidateError>),
}

/// Reexport of Edmx type to root.
pub type Edmx = edmx_root::Edmx;

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
