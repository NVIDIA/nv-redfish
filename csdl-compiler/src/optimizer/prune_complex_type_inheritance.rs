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

//! If there is only one child for complex type we can remove base
//! types and move all properties of base class to child class and
//! then remove unused based classes.
//!
//! We don't want touch parent classes with multiple chlidren because
//! in code generation it may be useful to have structure that represent
//! base class and flattened in during deserialization.
//!

use crate::compiler::Compiled;
use crate::compiler::ComplexType;
use crate::compiler::MapType as _;
use crate::compiler::Properties;
use crate::compiler::PropertiesManipulation as _;
use crate::compiler::Property;
use crate::compiler::QualifiedName;
use crate::optimizer::map_types_in_actions;
use crate::optimizer::replace;
use std::collections::HashMap;

pub fn prune_complex_type_inheritance<'a>(input: Compiled<'a>) -> Compiled<'a> {
    // 1. Create parent -> child map where parent have only one child.
    let single_child_parents = input
        .complex_types
        .iter()
        .fold(
            HashMap::<QualifiedName<'a>, (QualifiedName<'a>, u64)>::new(),
            |mut v, (_, ct)| {
                if let Some(base) = ct.base {
                    v.entry(base).or_insert((ct.name, 0)).1 += 1;
                }
                v
            },
        )
        .into_iter()
        .filter_map(|(parent, (child, cnt))| {
            if cnt == 1 {
                Some((parent, child))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();

    // 2. Create replacement mapping: parent -> most specific child.
    let replacements = single_child_parents
        .iter()
        .map(|(parent, mut child)| {
            while let Some(next) = single_child_parents.get(child) {
                child = next;
            }
            (*parent, *child)
        })
        .collect::<HashMap<_, _>>();

    // 3. Split complex types in two groups:
    //    a. Those that need to be removed
    //    b. Those that should retain
    let (mut remove, retain): (HashMap<_, _>, HashMap<_, _>) = input
        .complex_types
        .into_iter()
        .partition(|(name, _)| replacements.contains_key(name));

    let map_prop = |p: Property<'a>| p.map_type(|t| replace(&t, &replacements));
    Compiled {
        complex_types: retain
            .into_iter()
            // Pass all properties from single child parents to child.
            .map(|(name, v)| {
                let mut base = v.base;
                let mut properties = vec![v.properties];
                while let Some(next_base) = base {
                    if let Some(parent) = remove.remove(&next_base) {
                        properties.push(parent.properties);
                        base = parent.base;
                    } else {
                        break;
                    }
                }
                (
                    name,
                    ComplexType {
                        name: v.name,
                        base,
                        properties: Properties::rev_join(properties),
                        odata: v.odata,
                    },
                )
            })
            // Replace all names that can refer to parent classes
            .map(|(name, v)| (name, v.map_properties(map_prop)))
            .collect(),
        entity_types: input
            .entity_types
            .into_iter()
            .map(|(name, v)| (name, v.map_properties(map_prop)))
            .collect(),
        root_singletons: input.root_singletons,
        enum_types: input.enum_types,
        type_definitions: input.type_definitions,
        actions: map_types_in_actions(input.actions, |t| replace(&t, &replacements)),
    }
}
