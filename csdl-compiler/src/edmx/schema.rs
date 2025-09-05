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
use crate::edmx::EntityContainer;
use crate::edmx::SchemaNamespace;
use crate::edmx::Term;
use crate::edmx::TypeDefinition;
use crate::edmx::TypeName;
use crate::edmx::complex_type::ComplexType;
use crate::edmx::complex_type::DeComplexType;
use crate::edmx::entity_type::DeEntityType;
use crate::edmx::entity_type::EntityType;
use crate::edmx::enum_type::DeEnumType;
use crate::edmx::enum_type::EnumType;
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
    EntityType(DeEntityType),
    ComplexType(DeComplexType),
    EnumType(DeEnumType),
    TypeDefinition(TypeDefinition),
    EntityContainer(EntityContainer),
    Term(Term),
    Annotation(Annotation),
}

#[derive(Debug)]
pub enum Type {
    EntityType(EntityType),
    ComplexType(ComplexType),
    EnumType(EnumType),
    TypeDefinition(TypeDefinition),
    EntityContainer(EntityContainer),
    Term(Term),
}

#[derive(Debug)]
pub struct Schema {
    pub namespace: SchemaNamespace,
    pub types: HashMap<TypeName, Type>,
    pub annotations: Vec<Annotation>,
}

impl DeSchema {
    /// # Errors
    ///
    /// Returns error if any of items failed to validate.
    pub fn validate(self) -> Result<Schema, ValidateError> {
        let (types, annotations) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut ts, mut anns), v| {
                    match v {
                        DeSchemaItem::EntityType(v) => {
                            ts.push(v.validate().map(|v| (v.name.clone(), Type::EntityType(v))));
                        }
                        DeSchemaItem::ComplexType(v) => {
                            ts.push(v.validate().map(|v| (v.name.clone(), Type::ComplexType(v))));
                        }
                        DeSchemaItem::EnumType(v) => {
                            ts.push(v.validate().map(|v| (v.name.clone(), Type::EnumType(v))));
                        }
                        DeSchemaItem::TypeDefinition(v) => {
                            ts.push(Ok((v.name.clone(), Type::TypeDefinition(v))));
                        }
                        DeSchemaItem::EntityContainer(v) => {
                            ts.push(Ok((v.name.clone(), Type::EntityContainer(v))));
                        }
                        DeSchemaItem::Term(v) => {
                            ts.push(Ok((v.name.clone(), (Type::Term(v)))));
                        }
                        DeSchemaItem::Annotation(v) => anns.push(v),
                    }
                    (ts, anns)
                });
        let namespace = self.namespace;
        let types = types
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| ValidateError::Schema(namespace.clone(), Box::new(e)))?;

        Ok(Schema {
            namespace,
            types,
            annotations,
        })
    }
}
