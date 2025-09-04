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
use crate::edmx::ComplexType;
use crate::edmx::EntityContainer;
use crate::edmx::EntityType;
use crate::edmx::EnumType;
use crate::edmx::Term;
use crate::edmx::TypeDefinition;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct DeSchema {
    #[serde(rename = "@Namespace")]
    pub namespace: String,
    #[serde(rename = "@Alias")]
    pub alias: Option<String>,
    #[serde(rename = "$value", default)]
    pub items: Vec<DeSchemaItem>,
}

#[derive(Debug, Deserialize)]
pub enum DeSchemaItem {
    EntityType(EntityType),
    ComplexType(ComplexType),
    EnumType(EnumType),
    TypeDefinition(TypeDefinition),
    EntityContainer(EntityContainer),
    Term(Term),
    Annotation(Annotation),
}

pub enum Type {
    EntityType(EntityType),
    ComplexType(ComplexType),
    EnumType(EnumType),
    TypeDefinition(TypeDefinition),
    EntityContainer(EntityContainer),
    Term(Term),
}

pub struct Schema {
    pub types: HashMap<String, Type>,
    pub annotations: Vec<Annotation>,
}

impl DeSchema {
    /// # Errors
    /// Actually, doesn't return error but keep it consistent if it will.
    pub fn validate(self) -> Result<Schema, ValidateError> {
        let (types, annotations) =
            self.items
                .into_iter()
                .fold((HashMap::new(), Vec::new()), |(mut ts, mut anns), v| {
                    match v {
                        DeSchemaItem::EntityType(v) => {
                            ts.insert(v.name.clone(), Type::EntityType(v));
                        }
                        DeSchemaItem::ComplexType(v) => {
                            ts.insert(v.name.clone(), Type::ComplexType(v));
                        }
                        DeSchemaItem::EnumType(v) => {
                            ts.insert(v.name.clone(), Type::EnumType(v));
                        }
                        DeSchemaItem::TypeDefinition(v) => {
                            ts.insert(v.name.clone(), Type::TypeDefinition(v));
                        }
                        DeSchemaItem::EntityContainer(v) => {
                            ts.insert(v.name.clone(), Type::EntityContainer(v));
                        }
                        DeSchemaItem::Term(v) => {
                            ts.insert(v.name.clone(), Type::Term(v));
                        }
                        DeSchemaItem::Annotation(v) => anns.push(v),
                    }
                    (ts, anns)
                });

        Ok(Schema { types, annotations })
    }
}
