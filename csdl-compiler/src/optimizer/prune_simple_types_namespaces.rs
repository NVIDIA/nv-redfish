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

//! Prune simple types namespaces.
//!
//! Namespace has hierarchical structure (dot-separated).  This
//! optimization tries to move simple types to more higher namespace
//! keeping in account possible conflicts.
//!
//! This means that if type `T` is defined in namespace `A.v1_0_0` it will
//! be moved to namespace `A` if:
//! - No type `T` is defined in `A`
//! - No type `T` is defined in any other subnamespace of `A`.
//!

use crate::compiler::Compiled;
use crate::compiler::CompiledNamespace;
use crate::compiler::CompiledProperty;
use crate::compiler::MapType as _;
use crate::compiler::PropertiesManipulation as _;
use crate::compiler::QualifiedName;
use crate::edmx::attribute_values::SimpleIdentifier;
use std::collections::HashMap;

type NamespaceMatches<'a> = HashMap<CompiledNamespace<'a>, u64>;
type TypeNamespaces<'a> = HashMap<&'a SimpleIdentifier, NamespaceMatches<'a>>;

#[allow(clippy::elidable_lifetime_names)]
#[allow(clippy::missing_const_for_fn)]
pub fn prune_simple_types_namespaces<'a>(input: Compiled<'a>) -> Compiled<'a> {
    // 1. For each name we calculate statistics per parent
    // namespaces. How many occurances are in each namespace we have.
    let type_nss = input
        .simple_types
        .keys()
        .fold(TypeNamespaces::<'a>::new(), |mut map, name| {
            let matches = map.entry(name.name).or_default();
            let mut namespace = name.namespace;
            while let Some(parent) = namespace.parent() {
                *matches.entry(parent).or_insert(0) += 1;
                namespace = parent;
            }
            map
        });
    // 2. Find possible replacements
    let replacements = input
        .simple_types
        .keys()
        .filter_map(|orig| {
            type_nss.get(orig.name).and_then(|stats| {
                // Search through parent namespaces where this type name
                // is unique.
                let mut namespace = orig.namespace;
                let mut best = None;
                while let Some(parent) = namespace.parent() {
                    if let Some(cnt) = stats.get(&parent) {
                        if *cnt == 1 {
                            best = Some(parent);
                            namespace = parent;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                best.map(|best| {
                    (
                        *orig,
                        QualifiedName {
                            name: orig.name,
                            namespace: best,
                        },
                    )
                })
            })
        })
        .collect::<HashMap<_, _>>();
    let map_prop = |p: CompiledProperty<'a>| p.map_type(|t| super::replace(&t, &replacements));
    // 3. Apply replacements
    Compiled {
        simple_types: input
            .simple_types
            .into_iter()
            .filter(|(name, _)| !replacements.contains_key(name))
            .collect(),
        complex_types: input
            .complex_types
            .into_iter()
            .map(|(name, v)| (name, v.map_properties(map_prop)))
            .collect(),
        entity_types: input
            .entity_types
            .into_iter()
            .map(|(name, v)| (name, v.map_properties(map_prop)))
            .collect(),
        root_singletons: input.root_singletons,
    }
}
