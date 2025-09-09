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

//! EDMX parser and validator.

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

/// 10 Enumeration Type
pub mod enum_type;

/// 12.1 Element edm:Action
pub mod action;

/// 14.3 Element edm:Annotation
pub mod annotation;

/// 17 Attribute Values
pub mod attribute_values;

use annotation::Annotation;
use quick_xml::DeError;
use serde::Deserialize;
use tagged_types::TaggedType;

pub type TypeName = TaggedType<String, TypeNameTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq)]
#[transparent(Debug, Display, Deserialize)]
#[capability(inner_access)]
pub enum TypeNameTag {}

pub type TermName = TaggedType<String, TermNameTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq)]
#[transparent(Debug, Display, Deserialize)]
#[capability(inner_access)]
pub enum TermNameTag {}

pub type ActionName = TaggedType<String, TermNameTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Hash, PartialEq, Eq)]
#[transparent(Debug, Display, Deserialize)]
#[capability(inner_access)]
pub enum ActionNameTag {}

pub type SchemaNamespace = String;
pub type PropertyName = String;

pub type IsNullable = TaggedType<bool, IsNullableTag>;
#[derive(tagged_types::Tag)]
#[implement(Copy, Clone)]
#[transparent(Debug, Deserialize)]
#[capability(inner_access)]
pub enum IsNullableTag {}

pub type IsBound = TaggedType<bool, IsBoundTag>;
#[derive(tagged_types::Tag)]
#[implement(Copy, Clone)]
#[transparent(Debug, Deserialize)]
#[capability(inner_access)]
pub enum IsBoundTag {}

/// EDMX compilation errors.
#[derive(Debug)]
pub enum ValidateError {
    /// XML deserialization error.
    XmlDeserialize(DeError),
    /// Invalid number of `DataServices`.
    WrongDataServicesNumber,
    /// In the `EntityType` too many keys.
    TooManyKeys,
    /// In the `NavigationProperty` too many `OnDelete` items.
    TooManyOnDelete,
    /// In the `Action` too many `ReturnType` items.
    TooManyReturnTypes,
    /// Schema validation error.
    Schema(SchemaNamespace, Box<ValidateError>),
    /// `ComplexType` validation error.
    ComplexType(TypeName, Box<ValidateError>),
    /// `EntityType` validation error.
    EntityType(TypeName, Box<ValidateError>),
    /// `NavigationProperty` validation error.
    NavigationProperty(PropertyName, Box<ValidateError>),
    /// `Action` validation error.
    Action(ActionName, Box<ValidateError>),
}

/// Reexport of Edmx type to root.
pub type Edmx = edmx_root::Edmx;

#[derive(Debug, Deserialize)]
pub struct TypeDefinition {
    #[serde(rename = "@Name")]
    pub name: TypeName,
    #[serde(rename = "@UnderlyingType")]
    pub underlying_type: TypeName,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct EntityContainer {
    #[serde(rename = "@Name")]
    pub name: TypeName,
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
    pub name: TypeName,
    #[serde(rename = "@Type")]
    pub ttype: Option<String>,
    #[serde(rename = "@AppliesTo")]
    pub applies_to: Option<String>,
    #[serde(rename = "@DefaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

/// 12.3 Element edm:ReturnType
#[derive(Debug, Deserialize)]
pub struct ReturnType {
    /// 12.3.1 Attribute Type
    #[serde(rename = "@Type")]
    pub rtype: TypeName,
    /// 12.3.2 Attribute Nullable
    #[serde(rename = "@Nullable")]
    pub nullable: Option<IsNullable>,
}

/// 12.4 Element edm:Parameter
#[derive(Debug, Deserialize)]
pub struct Parameter {
    /// 12.4.1 Attribute Name
    #[serde(rename = "@Name")]
    pub name: String,
    /// 12.4.2 Attribute Type
    #[serde(rename = "@Type")]
    pub ptype: String,
    /// 12.4.3 Attribute Nullable
    #[serde(rename = "@Nullable")]
    pub nullable: Option<IsNullable>,
}
