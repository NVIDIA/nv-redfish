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

use crate::edmx::property::DeNavigationProperty;
use crate::edmx::Annotation;
use crate::edmx::LocalTypeName;
use crate::edmx::Property;
use crate::edmx::QualifiedTypeName;
use crate::edmx::StructuralProperty;
use crate::edmx::ValidateError;
use crate::IsAbstract;
use serde::Deserialize;

/// 9.1 Element edm:ComplexType
#[derive(Debug, Deserialize)]
pub struct DeComplexType {
    /// 9.1.1 Attribute `Name`
    #[serde(rename = "@Name")]
    pub name: LocalTypeName,
    /// 9.1.2 Attribute `BaseType`
    #[serde(rename = "@BaseType")]
    pub base_type: Option<QualifiedTypeName>,
    /// 9.1.3 Attribute `Abstract`
    #[serde(rename = "@Abstract")]
    pub r#abstract: Option<IsAbstract>,
    /// 9.1.4 Attribute `OpenType`
    #[serde(rename = "@OpenType")]
    pub open_type: Option<bool>,
    /// Items of edm:ComplexType
    #[serde(rename = "$value", default)]
    pub items: Vec<DeComplexTypeItem>,
}

/// Items of edm:ComplexType
#[derive(Debug, Deserialize)]
pub enum DeComplexTypeItem {
    #[serde(rename = "Property")]
    StructuralProperty(StructuralProperty),
    NavigationProperty(DeNavigationProperty),
    Annotation(Annotation),
}

/// Validated edm:ComplexType
#[derive(Debug)]
pub struct ComplexType {
    pub name: LocalTypeName,
    pub base_type: Option<QualifiedTypeName>,
    pub properties: Vec<Property>,
    pub annotations: Vec<Annotation>,
    pub is_abstract: IsAbstract,
}

impl DeComplexType {
    /// # Errors
    ///
    /// - `ValidateError::ComplexType` if error occured. Internal `ValidateError` contains details.
    pub fn validate(self) -> Result<ComplexType, ValidateError> {
        let (annotations, properties) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut anns, mut ps), v| {
                    match v {
                        DeComplexTypeItem::StructuralProperty(p) => ps.push(p.validate()),
                        DeComplexTypeItem::NavigationProperty(p) => ps.push(p.validate()),
                        DeComplexTypeItem::Annotation(a) => anns.push(a),
                    }
                    (anns, ps)
                });
        let name = self.name;
        let properties = properties
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ValidateError::ComplexType(name.clone(), Box::new(e)))?;
        Ok(ComplexType {
            name,
            base_type: self.base_type,
            properties,
            annotations,
            is_abstract: self.r#abstract.unwrap_or(IsAbstract::new(false)),
        })
    }
}
