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
use crate::compiler::Error;
use crate::compiler::MapBase;
use crate::compiler::NavProperty;
use crate::compiler::OData;
use crate::compiler::Properties;
use crate::compiler::PropertiesManipulation;
use crate::compiler::Property;
use crate::compiler::QualifiedName;
use crate::compiler::SchemaIndex;
use crate::compiler::Stack;
use crate::compiler::odata::MustHaveId;
use crate::edmx::entity_type::EntityType as EdmxEntityType;
use crate::edmx::entity_type::Key;

#[derive(Debug)]
pub struct EntityType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub key: Option<&'a Key>,
    pub properties: Properties<'a>,
    pub odata: OData<'a>,
}

impl<'a> EntityType<'a> {
    /// Compiles entity type with specified name. Note that it also
    /// compiles all dependencies of the enity type.
    ///
    /// # Errors
    ///
    /// Returns error if failed to compile any prerequisites of the
    /// `schema_entity_type`.
    pub fn compile(
        name: QualifiedName<'a>,
        schema_entity_type: &'a EdmxEntityType,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let stack = stack.new_frame().with_enitity_type(name);
        // Ensure that base entity type compiled if present.
        let (base, compiled) = if let Some(base_type) = &schema_entity_type.base_type {
            let compiled = Self::ensure(base_type.into(), schema_index, &stack)?;
            (Some(base_type.into()), compiled)
        } else {
            (None, Compiled::default())
        };
        let stack = stack.new_frame().merge(compiled);

        // Compile navigation and regular properties
        let (compiled, properties) = Properties::compile(
            &schema_entity_type.properties,
            schema_index,
            stack.new_frame(),
        )?;

        Ok(stack
            .merge(compiled)
            .merge(Compiled::new_entity_type(EntityType {
                name,
                base,
                key: schema_entity_type.key.as_ref(),
                properties,
                odata: OData::new(MustHaveId::new(true), schema_entity_type),
            }))
            .done())
    }

    /// Checks if `EntityType` with name `qtype` is compiled. If not
    /// then compile it.
    ///
    /// # Errors
    ///
    /// Returns error if failed to compile entity type.
    pub fn ensure(
        qtype: QualifiedName<'a>,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        if stack.contains_entity(qtype) {
            Ok(Compiled::default())
        } else {
            schema_index
                .find_entity_type(qtype)
                .ok_or(Error::EntityTypeNotFound(qtype))
                .and_then(|et| Self::compile(qtype, et, schema_index, stack))
                .map_err(Box::new)
                .map_err(|e| Error::EntityType(qtype, e))
        }
    }
}

impl<'a> PropertiesManipulation<'a> for EntityType<'a> {
    fn map_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(Property<'a>) -> Property<'a>,
    {
        self.properties.properties = self.properties.properties.into_iter().map(f).collect();
        self
    }

    fn map_nav_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(NavProperty<'a>) -> NavProperty<'a>,
    {
        self.properties.nav_properties =
            self.properties.nav_properties.into_iter().map(f).collect();
        self
    }
}

impl<'a> MapBase<'a> for EntityType<'a> {
    fn map_base<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.base = self.base.map(f);
        self
    }
}
