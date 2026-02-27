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

use crate::compiler::odata::MustHaveId;
use crate::compiler::Compiled;
use crate::compiler::Context;
use crate::compiler::Error;
use crate::compiler::MapBase;
use crate::compiler::NavProperty;
use crate::compiler::OData;
use crate::compiler::Properties;
use crate::compiler::PropertiesManipulation;
use crate::compiler::Property;
use crate::compiler::QualifiedName;
use crate::compiler::Stack;
use crate::edmx::entity_type::Key;
use crate::edmx::EntityType as EdmxEntityType;
use crate::IsAbstract;

/// Compiled entity type.
#[derive(Debug)]
pub struct EntityType<'a> {
    /// Fully qualified type name.
    pub name: QualifiedName<'a>,
    /// Optional base entity type.
    pub base: Option<QualifiedName<'a>>,
    /// Optional key definition.
    pub key: Option<&'a Key>,
    /// Structural and navigation properties.
    pub properties: Properties<'a>,
    /// Attached `OData` annotations.
    pub odata: OData<'a>,
    /// Whether the type is abstract.
    pub is_abstract: IsAbstract,
}

impl<'a> EntityType<'a> {
    /// Compile an `EntityType` with the specified name, including all
    /// of its dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error if any prerequisite of `schema_entity_type`
    /// fails to compile.
    pub fn compile(
        name: QualifiedName<'a>,
        schema_entity_type: &'a EdmxEntityType,
        ctx: &Context<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let stack = stack.new_frame().with_enitity_type(name);
        // Ensure that base entity type compiled if present.
        let (base, compiled) = if let Some(base_type) = &schema_entity_type.base_type {
            let compiled = Self::ensure(base_type.into(), ctx, &stack)?;
            (Some(base_type.into()), compiled)
        } else {
            (None, Compiled::default())
        };
        let stack = stack.new_frame().merge(compiled);

        // Compile navigation and regular properties
        let (compiled, properties) =
            Properties::compile(name, &schema_entity_type.properties, ctx, stack.new_frame())?;

        let entity_type = EntityType {
            name,
            base,
            key: schema_entity_type.key.as_ref(),
            properties,
            odata: OData::new(MustHaveId::new(true), schema_entity_type),
            is_abstract: schema_entity_type.is_abstract,
        };
        Ok(stack
            .merge(compiled)
            .merge(Compiled::new_entity_type(entity_type))
            .done())
    }

    /// Ensure that the `EntityType` named `qtype` is compiled; compile
    /// it if not already present.
    ///
    /// # Errors
    ///
    /// Returns an error if compiling the entity type fails.
    pub fn ensure(
        qtype: QualifiedName<'a>,
        ctx: &Context<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        if stack.contains_entity(qtype) {
            Ok(Compiled::default())
        } else {
            ctx.schema_index
                .find_entity_type(qtype)
                .ok_or(Error::EntityTypeNotFound(qtype))
                .and_then(|et| Self::compile(qtype, et, ctx, stack))
                .map_err(Box::new)
                .map_err(|e| Error::EntityType(qtype, e))
        }
    }

    /// Insertable collection member type.
    ///
    /// For collections marked `Insertable`, returns the member type
    /// name.
    ///
    #[must_use]
    pub fn insertable_member_type(&self) -> Option<QualifiedName<'a>> {
        if self.odata.insertable.is_some_and(|v| v.inner().value) {
            self.properties
                .nav_properties
                .iter()
                .find(|p| p.name().inner().inner() == "Members")
                .and_then(|p| match p {
                    NavProperty::Expandable(v) => Some(v),
                    NavProperty::Reference(_) => None,
                })
                .map(|p| p.ptype.name())
        } else {
            None
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
