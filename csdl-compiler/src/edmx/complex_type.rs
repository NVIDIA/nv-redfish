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
use crate::edmx::TypeName;
use crate::edmx::property::DeNavigationProperty;
use crate::edmx::property::DeStructuralProperty;
use crate::edmx::property::Property;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DeComplexType {
    #[serde(rename = "@Name")]
    pub name: TypeName,
    #[serde(rename = "@BaseType")]
    pub base_type: Option<TypeName>,
    #[serde(rename = "@Abstract")]
    pub r#abstract: Option<bool>,
    #[serde(rename = "@OpenType")]
    pub open_type: Option<bool>,
    #[serde(rename = "@HasStream")]
    pub has_stream: Option<bool>,
    #[serde(rename = "$value", default)]
    pub items: Vec<DeComplexTypeItem>,
}

#[derive(Debug, Deserialize)]
pub enum DeComplexTypeItem {
    #[serde(rename = "Property")]
    StructuralProperty(DeStructuralProperty),
    NavigationProperty(DeNavigationProperty),
    Annotation(Annotation),
}

#[derive(Debug)]
pub struct ComplexType {
    pub name: TypeName,
    pub properties: Vec<Property>,
    pub annotations: Vec<Annotation>,
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
            properties,
            annotations,
        })
    }
}
