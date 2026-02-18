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

//! This is collection of optimizations of compiled data structures.
//! Each submodule of this module contains one or more optimizations
//! combinded by same topic.
//!
//! Any optimization function is function that consumes old compiled
//! data structure and produce new, optimized one.

mod prune_complex_type_inheritance;
mod prune_entity_type_inheritance;
mod prune_namespaces;
mod remove_empty_complex_types;
mod remove_empty_entity_types;

use crate::compiler::Compiled;
use crate::compiler::EntityTypeFilter;
use crate::compiler::MapType as _;
use crate::compiler::QualifiedName;
use crate::compiler::TypeActions;
use prune_complex_type_inheritance::prune_complex_type_inheritance;
use prune_entity_type_inheritance::prune_entity_type_inheritance;
use prune_namespaces::prune_namespaces;
use remove_empty_complex_types::remove_empty_complex_types;
use remove_empty_entity_types::remove_empty_entity_types;
use std::collections::HashMap;

pub struct Config {
    pub never_prune: EntityTypeFilter,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            never_prune: EntityTypeFilter::new_permissive(
                ["Resource.Item", "Resource.ItemOrCollection"]
                    .iter()
                    .map(|f| f.parse().expect("must be correct filter"))
                    .collect(),
            ),
        }
    }
}

/// Apply all known optimizations to compiled data structures.
#[must_use]
pub fn optimize<'a>(input: Compiled<'a>, config: &Config) -> Compiled<'a> {
    [
        remove_empty_complex_types,
        remove_empty_entity_types,
        prune_complex_type_inheritance,
        prune_entity_type_inheritance,
        prune_namespaces,
    ]
    .iter()
    .fold(input, |input, f| f(input, config))
}

type Replacements<'a> = HashMap<QualifiedName<'a>, QualifiedName<'a>>;

fn replace<'a>(target: &QualifiedName<'a>, replacements: &Replacements<'a>) -> QualifiedName<'a> {
    *replacements.get(target).unwrap_or(target)
}

pub fn map_types_in_actions<'a, F>(actions: TypeActions<'a>, f: F) -> TypeActions<'a>
where
    F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
{
    actions
        .into_iter()
        .map(|(qname, actions)| {
            (
                f(qname),
                actions
                    .into_iter()
                    .map(|(aname, compiled_action)| (aname, compiled_action.map_type(&f)))
                    .collect(),
            )
        })
        .collect()
}
