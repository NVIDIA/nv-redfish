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

use crate::ValidateError;
use crate::edmx::Annotation;
use crate::edmx::OnDelete;
use crate::edmx::PropertyName;
use crate::edmx::ReferentialConstraint;
use crate::edmx::TypeName;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DeStructuralProperty {
    #[serde(rename = "@Name")]
    pub name: PropertyName,
    #[serde(rename = "@Type")]
    pub ptype: TypeName,
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
pub struct DeNavigationProperty {
    #[serde(rename = "@Name")]
    pub name: PropertyName,
    #[serde(rename = "@Type")]
    pub ptype: TypeName,
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

#[derive(Debug)]
pub struct Property {
    pub name: PropertyName,
    pub attrs: PropertyAttrs,
}

#[derive(Debug)]
pub enum PropertyAttrs {
    StructuralProperty(DeStructuralProperty),
    NavigationProperty(DeNavigationProperty),
}

impl DeStructuralProperty {
    /// # Errors
    ///
    /// Actually, doesn't return any errors. Keep it for consistency.
    pub fn validate(self) -> Result<Property, ValidateError> {
        Ok(Property {
            name: self.name.clone(),
            attrs: PropertyAttrs::StructuralProperty(self),
        })
    }
}

impl DeNavigationProperty {
    /// # Errors
    ///
    /// Actually, doesn't return any errors. Keep it for consistency.
    pub fn validate(self) -> Result<Property, ValidateError> {
        Ok(Property {
            name: self.name.clone(),
            attrs: PropertyAttrs::NavigationProperty(self),
        })
    }
}
