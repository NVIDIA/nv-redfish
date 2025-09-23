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

//! Prune namespaces.
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

mod complex_types;
mod entity_types;
mod enum_types;
mod type_definitions;

use crate::compiler::Compiled;
use crate::compiler::Namespace;
use crate::compiler::QualifiedName;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::optimizer::Replacements;
use std::collections::HashMap;

#[must_use]
pub fn prune_namespaces(input: Compiled<'_>) -> Compiled<'_> {
    [
        enum_types::prune,
        type_definitions::prune,
        complex_types::prune,
        entity_types::prune,
    ]
    .iter()
    .fold(input, |input, f| f(input))
}

type NamespaceMatches<'a> = HashMap<Namespace<'a>, u64>;
type TypeNamespaces<'a> = HashMap<&'a SimpleIdentifier, NamespaceMatches<'a>>;

fn prune_namepaces_replacements<'a, F, I>(f: F) -> Replacements<'a>
where
    I: Iterator<Item = QualifiedName<'a>>,
    F: Fn() -> I,
{
    // 1. For each name we calculate statistics per parent
    // namespaces. How many occurances are in each namespace we have.
    let type_nss = f().fold(TypeNamespaces::<'a>::new(), |mut map, name| {
        let matches = map.entry(name.name).or_default();
        let mut namespace = name.namespace;
        while let Some(parent) = namespace.parent() {
            *matches.entry(parent).or_insert(0) += 1;
            namespace = parent;
        }
        map
    });
    // 2. Find possible replacements
    f().filter_map(|orig| {
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
                    orig,
                    QualifiedName {
                        name: orig.name,
                        namespace: best,
                    },
                )
            })
        })
    })
    .collect::<HashMap<_, _>>()
}
