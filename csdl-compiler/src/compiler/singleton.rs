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

use crate::compiler::Compiled;
use crate::compiler::EntityType;
use crate::compiler::Error;
use crate::compiler::MapType;
use crate::compiler::QualifiedName;
use crate::compiler::SchemaIndex;
use crate::compiler::Stack;
use crate::edmx::Singleton as EdmxSingleton;
use crate::edmx::attribute_values::SimpleIdentifier;

#[derive(Debug)]
pub struct Singleton<'a> {
    pub name: &'a SimpleIdentifier,
    pub stype: QualifiedName<'a>,
}

impl<'a> Singleton<'a> {
    /// # Errors
    ///
    /// Returns `Error::Singleton` error if failed to compile entity
    /// type of the singleton.
    pub fn compile(
        singleton: &'a EdmxSingleton,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        schema_index
            // We are searching for deepest available child in tre
            // hierarchy of types for singleton. So, we can parse most
            // recent protocol versions.
            .find_child_entity_type((&singleton.stype).into())
            .and_then(|(qtype, et)| {
                if stack.contains_entity(qtype) {
                    // Aready compiled singleton
                    Ok(Compiled::default())
                } else {
                    EntityType::compile(qtype, et, schema_index, stack)
                        .map_err(Box::new)
                        .map_err(|e| Error::EntityType(qtype, e))
                }
                .map(|compiled| (qtype, compiled))
            })
            .map_err(Box::new)
            .map_err(|e| Error::Singleton(&singleton.name, e))
            .map(|(qtype, compiled)| {
                compiled.merge(Compiled::new_singleton(Singleton {
                    name: &singleton.name,
                    stype: qtype,
                }))
            })
    }
}

impl<'a> MapType<'a> for Singleton<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.stype = f(self.stype);
        self
    }
}
