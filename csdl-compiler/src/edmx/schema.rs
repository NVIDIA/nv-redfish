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
use crate::edmx::EntityContainer;
use crate::edmx::Namespace;
use crate::edmx::SimpleIdentifier;
use crate::edmx::Term;
use crate::edmx::TypeDefinition;
use crate::edmx::action::Action;
use crate::edmx::action::DeAction;
use crate::edmx::annotation::Annotation;
use crate::edmx::complex_type::ComplexType;
use crate::edmx::complex_type::DeComplexType;
use crate::edmx::entity_type::DeEntityType;
use crate::edmx::entity_type::EntityType;
use crate::edmx::enum_type::DeEnumType;
use crate::edmx::enum_type::EnumType;
use serde::Deserialize;
use std::collections::HashMap;

/// 5.1 Element edm:Schema
///
/// This is object for deserialization.
#[derive(Debug, Deserialize)]
pub struct DeSchema {
    /// 5.1.1 Attribute Namespace
    #[serde(rename = "@Namespace")]
    pub namespace: Namespace,
    /// Children of schema.
    #[serde(rename = "$value", default)]
    pub items: Vec<DeSchemaItem>,
}

/// Deserialization of schema children.
#[derive(Debug, Deserialize)]
pub enum DeSchemaItem {
    EntityType(DeEntityType),
    ComplexType(DeComplexType),
    EnumType(DeEnumType),
    TypeDefinition(TypeDefinition),
    EntityContainer(EntityContainer),
    Term(Term),
    Annotation(Annotation),
    Action(DeAction),
}

#[derive(Debug)]
pub enum Type {
    ComplexType(ComplexType),
    EnumType(EnumType),
    TypeDefinition(TypeDefinition),
}

/// Validated schema.
#[derive(Debug)]
pub struct Schema {
    pub namespace: Namespace,
    pub entity_types: HashMap<SimpleIdentifier, EntityType>,
    pub types: HashMap<SimpleIdentifier, Type>,
    pub terms: HashMap<SimpleIdentifier, Term>,
    pub entity_container: Option<EntityContainer>,
    pub actions: Vec<Action>,
    pub annotations: Vec<Annotation>,
}

impl DeSchema {
    /// # Errors
    ///
    /// Returns error if any of items failed to validate.
    pub fn validate(self) -> Result<Schema, ValidateError> {
        let (types, entity_types, annotations, terms, actions, mut entity_containers) =
            self.items.into_iter().fold(
                (
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                |(mut ts, mut ets, mut anns, mut terms, mut acts, mut ecs), v| {
                    match v {
                        DeSchemaItem::EntityType(v) => {
                            ets.push(v.validate().map(|v| (v.name.clone().into_inner(), v)));
                        }
                        DeSchemaItem::ComplexType(v) => {
                            ts.push(
                                v.validate()
                                    .map(|v| (v.name.clone().into_inner(), Type::ComplexType(v))),
                            );
                        }
                        DeSchemaItem::EnumType(v) => {
                            ts.push(
                                v.validate()
                                    .map(|v| (v.name.clone().into_inner(), Type::EnumType(v))),
                            );
                        }
                        DeSchemaItem::TypeDefinition(v) => {
                            ts.push(Ok((v.name.clone().into_inner(), Type::TypeDefinition(v))));
                        }
                        DeSchemaItem::EntityContainer(v) => {
                            ecs.push(v);
                        }
                        DeSchemaItem::Term(v) => {
                            terms.push(Ok((v.name.clone().into_inner(), v)));
                        }
                        DeSchemaItem::Annotation(v) => anns.push(v),
                        DeSchemaItem::Action(v) => acts.push(v.validate()),
                    }
                    (ts, ets, anns, terms, acts, ecs)
                },
            );
        let namespace = self.namespace;
        let types = types
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| ValidateError::Schema(namespace.clone(), Box::new(e)))?;

        let entity_types = entity_types
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| ValidateError::Schema(namespace.clone(), Box::new(e)))?;

        let actions = actions
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ValidateError::Schema(namespace.clone(), Box::new(e)))?;

        let terms = terms
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| ValidateError::Schema(namespace.clone(), Box::new(e)))?;

        if entity_containers.len() > 1 {
            return Err(ValidateError::Schema(
                namespace,
                Box::new(ValidateError::ManyContainersNotSupported),
            ));
        }
        let entity_container = entity_containers.pop();

        Ok(Schema {
            namespace,
            entity_types,
            types,
            terms,
            entity_container,
            actions,
            annotations,
        })
    }
}
