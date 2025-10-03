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

use crate::IsNullable;
use crate::edmx::Annotation;
use crate::edmx::PropertyName;
use crate::edmx::TypeName;
use crate::edmx::ValidateError;
use serde::Deserialize;

/// 6.1 Element edm:Property
#[derive(Debug, Deserialize)]
pub struct StructuralProperty {
    /// 6.1.1 Attribute `Name`
    #[serde(rename = "@Name")]
    pub name: PropertyName,
    /// 6.1.2 Attribute `Type`
    #[serde(rename = "@Type")]
    pub ptype: TypeName,
    /// 6.2.1 Attribute `Nullable`
    #[serde(rename = "@Nullable")]
    pub nullable: Option<IsNullable>,
    /// 6.2.2 Attribute `MaxLength`
    #[serde(rename = "@MaxLength")]
    pub max_length: Option<String>,
    /// 6.2.3 Attribute `Precision`
    #[serde(rename = "@Precision")]
    pub precision: Option<i32>,
    /// 6.2.4 Attribute `Scale`
    #[serde(rename = "@Scale")]
    pub scale: Option<String>,
    /// 6.2.5 Attribute `Unicode`
    #[serde(rename = "@Unicode")]
    pub unicode: Option<bool>,
    /// 6.2.6 Attribute `SRID`
    /// Non-negative integer or special value `variable`.
    #[serde(rename = "@SRID")]
    pub srid: Option<String>,
    /// 6.2.7 Attribute `DefaultValue`
    #[serde(rename = "@DefaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

/// 7.1 Element edm:NavigationProperty
#[derive(Debug, Deserialize)]
pub struct DeNavigationProperty {
    /// 7.1.1 Attribute `Name`
    #[serde(rename = "@Name")]
    pub name: PropertyName,
    /// 7.1.2 Attribute `Type`
    #[serde(rename = "@Type")]
    pub ptype: TypeName,
    /// 7.1.3 Attribute `Nullable`
    #[serde(rename = "@Nullable")]
    pub nullable: Option<IsNullable>,
    /// 7.1.4 Attribute `Partner`
    #[serde(rename = "@Partner")]
    pub partner: Option<String>,
    /// 7.1.5 Attribute `ContainsTarget`
    #[serde(rename = "@ContainsTarget")]
    pub contains_target: Option<bool>,
    /// Items of edm:NavigationProperty
    #[serde(rename = "$value", default)]
    pub items: Vec<DeNavigationPropertyItem>,
}

/// Items of edm:NavigationProperty
#[derive(Debug, Deserialize)]
pub enum DeNavigationPropertyItem {
    /// 7.2 Element edm:ReferentialConstraint
    ReferentialConstraint(ReferentialConstraint),
    /// 7.3 Element edm:OnDelete
    OnDelete(OnDelete),
    /// Annotations can be in any property.
    Annotation(Annotation),
}

/// 7.2 Element edm:ReferentialConstraint
#[derive(Debug, Deserialize)]
pub struct ReferentialConstraint {
    /// 7.2.1 Attribute `Property`
    #[serde(rename = "@Property")]
    pub property: String,
    /// 7.2.2 Attribute `ReferencedProperty`
    #[serde(rename = "@ReferencedProperty")]
    pub referenced_property: String,
}

/// 7.3 Element edm:OnDelete
#[derive(Debug, Deserialize)]
pub struct OnDelete {
    /// 7.3.1 Attribute Action
    #[serde(rename = "@Action")]
    pub action: String,
}

/// Validated element of edm:NavigationProperty or edm:Property
#[derive(Debug)]
pub struct Property {
    /// Name of the property.
    pub name: PropertyName,
    /// Attributes of the property.
    pub attrs: PropertyAttrs,
}

/// Attributes of the property.
#[derive(Debug)]
pub enum PropertyAttrs {
    /// Properties of the structural property.
    StructuralProperty(StructuralProperty),
    /// Properties of the navigation property.
    NavigationProperty(NavigationProperty),
}

impl StructuralProperty {
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
    /// `ValidateError::NavigationProperty` error if more than one edmx:OnDelete specified.
    pub fn validate(self) -> Result<Property, ValidateError> {
        let (mut on_deletes, referential_constraints, annotations) = self.items.into_iter().fold(
            (Vec::new(), Vec::new(), Vec::new()),
            |(mut dels, mut rcs, mut anns), v| {
                match v {
                    DeNavigationPropertyItem::OnDelete(v) => dels.push(v),
                    DeNavigationPropertyItem::ReferentialConstraint(v) => rcs.push(v),
                    DeNavigationPropertyItem::Annotation(v) => anns.push(v),
                }
                (dels, rcs, anns)
            },
        );
        if on_deletes.len() > 1 {
            return Err(ValidateError::NavigationProperty(
                self.name,
                Box::new(ValidateError::TooManyOnDelete),
            ));
        }
        let on_delete = on_deletes.pop();
        Ok(Property {
            name: self.name.clone(),
            attrs: PropertyAttrs::NavigationProperty(NavigationProperty {
                name: self.name,
                ptype: self.ptype,
                nullable: self.nullable,
                partner: self.partner,
                contains_target: self.contains_target,
                annotations,
                on_delete,
                referential_constraints,
            }),
        })
    }
}

/// Validated navigation property.
#[derive(Debug)]
pub struct NavigationProperty {
    pub name: PropertyName,
    pub ptype: TypeName,
    pub nullable: Option<IsNullable>,
    pub partner: Option<String>,
    pub contains_target: Option<bool>,
    pub annotations: Vec<Annotation>,
    pub on_delete: Option<OnDelete>,
    pub referential_constraints: Vec<ReferentialConstraint>,
}
