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
use crate::edmx::Key;
use crate::edmx::TypeName;
use crate::edmx::property::NavigationProperty;
use crate::edmx::property::Property;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DeEntityType {
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
    pub items: Vec<DeEntityTypeItem>,
}

#[derive(Debug, Deserialize)]
pub enum DeEntityTypeItem {
    Key(Key),
    Property(Property),
    NavigationProperty(NavigationProperty),
    Annotation(Annotation),
}

#[derive(Debug)]
pub struct EntityType {
    pub name: TypeName,
    pub key: Option<Key>,
    pub annotations: Vec<Annotation>,
}

impl DeEntityType {
    /// # Errors
    ///
    /// - `ValidateError::TooManyKeys` if more than one key is specified
    pub fn validate(self) -> Result<EntityType, ValidateError> {
        let (keys, annotations) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut keys, mut anns), v| {
                    match v {
                        DeEntityTypeItem::Key(k) => {
                            keys.push(k);
                        }
                        DeEntityTypeItem::Property(_) | DeEntityTypeItem::NavigationProperty(_) => {
                        }
                        DeEntityTypeItem::Annotation(a) => anns.push(a),
                    }
                    (keys, anns)
                });
        if keys.len() > 1 {
            return Err(ValidateError::TooManyKeys(self.name));
        }
        let key = keys.into_iter().next();
        Ok(EntityType {
            name: self.name,
            key,
            annotations,
        })
    }
}
