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

use crate::compiler::Action;
use crate::compiler::ComplexType;
use crate::compiler::EntityType;
use crate::compiler::EnumType;
use crate::compiler::QualifiedName;
use crate::compiler::TypeDefinition;
use crate::edmx::ActionName;
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::once as iter_once;
use tagged_types::TaggedType;

pub type ActionsMap<'a> = HashMap<&'a ActionName, Action<'a>>;
pub type TypeActions<'a> = HashMap<QualifiedName<'a>, ActionsMap<'a>>;

/// Flag that type is creatable
pub type IsCreatable = TaggedType<bool, IsCreatableTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy)]
#[transparent(Debug)]
#[capability(inner_access)]
pub enum IsCreatableTag {}

/// Compiled data from schema.
#[derive(Default, Debug)]
pub struct Compiled<'a> {
    pub complex_types: HashMap<QualifiedName<'a>, ComplexType<'a>>,
    pub entity_types: HashMap<QualifiedName<'a>, EntityType<'a>>,
    pub type_definitions: HashMap<QualifiedName<'a>, TypeDefinition<'a>>,
    pub enum_types: HashMap<QualifiedName<'a>, EnumType<'a>>,
    pub actions: TypeActions<'a>,
    pub creatable_entity_types: HashSet<QualifiedName<'a>>,
}

impl<'a> Compiled<'a> {
    /// Creates compiled data structure that contains only one compiled
    /// entity type.
    #[must_use]
    pub fn new_entity_type(v: EntityType<'a>) -> Self {
        let creatable_entity_types = v
            .insertable_member_type()
            .map_or_else(HashSet::new, |insertable_type| {
                iter_once(&insertable_type).copied().collect()
            });
        Self {
            entity_types: vec![(v.name, v)].into_iter().collect(),
            creatable_entity_types,
            ..Default::default()
        }
    }

    /// Creates compiled data structure that contains only one compiled
    /// complex type.
    #[must_use]
    pub fn new_complex_type(v: ComplexType<'a>) -> Self {
        Self {
            complex_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates compiled data structure that contains only one type
    /// definition.
    #[must_use]
    pub fn new_type_definition(v: TypeDefinition<'a>) -> Self {
        Self {
            type_definitions: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates compiled data structure that contains only one enum
    /// type.
    #[must_use]
    pub fn new_enum_type(v: EnumType<'a>) -> Self {
        Self {
            enum_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates compiled data structure that contains only one enum
    /// type.
    #[must_use]
    pub fn new_action(v: Action<'a>) -> Self {
        Self {
            actions: vec![(v.binding, vec![(v.name, v)].into_iter().collect())]
                .into_iter()
                .collect(),
            ..Default::default()
        }
    }

    /// Merge two compiled data structures.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.complex_types.extend(other.complex_types);
        self.type_definitions.extend(other.type_definitions);
        self.enum_types.extend(other.enum_types);
        self.entity_types.extend(other.entity_types);
        self.creatable_entity_types
            .extend(other.creatable_entity_types);
        self.actions =
            other
                .actions
                .into_iter()
                .fold(self.actions, |mut selfactions, (qname, actions)| {
                    let new_actions = match selfactions.remove(&qname) {
                        None => actions,
                        Some(mut v) => {
                            v.extend(actions);
                            v
                        }
                    };
                    selfactions.insert(qname, new_actions);
                    selfactions
                });
        self
    }
}
