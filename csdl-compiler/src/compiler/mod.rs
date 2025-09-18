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

//! Compiler of multiple schemas

/// Index of schemas
pub mod schema_index;

/// Compilation stack
pub mod stack;

/// Error diagnostics
pub mod error;

/// Compiled schema bundle
pub mod compiled;

/// Qualified name
pub mod qualified_name;

/// Compiled namespace
pub mod namespace;

/// Compiled odata
pub mod odata;

/// Compiled properties of `ComplexType` or `EntityType`
pub mod compiled_properties;

/// Simple type (type definition or enum)
pub mod simple_type;

use crate::edmx::Edmx;
use crate::edmx::PropertyName;
use crate::edmx::QualifiedTypeName;
use crate::edmx::Singleton;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::edmx::attribute_values::TypeName;
use crate::edmx::entity_type::EntityType;
use crate::edmx::entity_type::Key;
use crate::edmx::property::Property;
use crate::edmx::property::PropertyAttrs;
use crate::edmx::schema::Schema;
use crate::edmx::schema::Type;
use schema_index::SchemaIndex;
use stack::Stack;

/// Reexport `Compiled` to the level of the compiler.
pub type Compiled<'a> = compiled::Compiled<'a>;
/// Reexport `Error` to the level of the compiler.
pub type Error<'a> = error::Error<'a>;
/// Reexport `QualifiedName` to the level of the compiler.
pub type QualifiedName<'a> = qualified_name::QualifiedName<'a>;
/// Reexport `CompiledNamespace` to the level of the compiler.
pub type CompiledNamespace<'a> = namespace::CompiledNamespace<'a>;
/// Reexport `CompiledOData` to the level of the compiler.
pub type CompiledOData<'a> = odata::CompiledOData<'a>;
/// Reexport `CompiledProperties` to the level of the compiler.
pub type CompiledProperties<'a> = compiled_properties::CompiledProperties<'a>;
/// Reexport `SimpleType` to the level of the compiler.
pub type SimpleType<'a> = simple_type::SimpleType<'a>;
/// Reexport `SimpleTypeAttrs` to the level of the compiler.
pub type SimpleTypeAttrs<'a> = simple_type::SimpleTypeAttrs<'a>;
/// Reexport `CompiledTypeDefinition` to the level of the compiler.
pub type CompiledTypeDefinition<'a> = simple_type::CompiledTypeDefinition<'a>;
/// Reexport `CompiledEnumType` to the level of the compiler.
pub type CompiledEnumType<'a> = simple_type::CompiledEnumType<'a>;

/// Collection of EDMX documents that are compiled together to produce
/// code.
#[derive(Default)]
pub struct SchemaBundle {
    /// Parsed and validated Edmx documents.
    pub edmx_docs: Vec<Edmx>,
}

impl SchemaBundle {
    /// Compile multiple schema, resolving all type dependencies.
    ///
    /// # Errors
    ///
    /// Returns compile error if any type cannot be resolved.
    pub fn compile(&self, singletons: &[SimpleIdentifier]) -> Result<Compiled<'_>, Error> {
        let schema_index = SchemaIndex::build(&self.edmx_docs);
        let stack = Stack::default();
        self.edmx_docs
            .iter()
            .try_fold(stack, |stack, edmx| {
                let cstack = stack.new_frame();
                let compiled = edmx
                    .data_services
                    .schemas
                    .iter()
                    .try_fold(cstack, |stack, s| {
                        Self::compile_schema(s, singletons, &schema_index, stack.new_frame())
                            .map(|v| stack.merge(v))
                    })?
                    .done();
                Ok(stack.merge(compiled))
            })
            .map(Stack::done)
    }

    fn compile_schema<'a>(
        s: &'a Schema,
        singletons: &[SimpleIdentifier],
        schema_index: &SchemaIndex<'a>,
        stack: Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        s.entity_container.as_ref().map_or_else(
            || Ok(Compiled::default()),
            |entity_container| {
                entity_container
                    .singletons
                    .iter()
                    .try_fold(stack, |stack, s| {
                        if singletons.contains(&s.name) {
                            Self::compile_singleton(s, schema_index, &stack).map(|v| stack.merge(v))
                        } else {
                            Ok(stack)
                        }
                    })
                    .map_err(Box::new)
                    .map_err(|e| Error::Schema(&s.namespace, e))
                    .map(Stack::done)
            },
        )
    }

    fn compile_singleton<'a>(
        singleton: &'a Singleton,
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
                    Self::compile_entity_type(qtype, et, schema_index, stack)
                        .map_err(Box::new)
                        .map_err(|e| Error::EntityType(qtype, e))
                }
                .map(|compiled| (qtype, compiled))
            })
            .map_err(Box::new)
            .map_err(|e| Error::Singleton(&singleton.name, e))
            .map(|(qtype, compiled)| {
                compiled.merge(Compiled::new_singleton(CompiledSingleton {
                    name: &singleton.name,
                    stype: qtype,
                }))
            })
    }

    fn ensure_entity_type<'a>(
        qtype: &'a QualifiedTypeName,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        if stack.contains_entity(qtype.into()) {
            Ok(Compiled::default())
        } else {
            Self::find_and_compile_entity_type(qtype, schema_index, stack)
        }
    }

    fn find_and_compile_entity_type<'a>(
        qtype: &'a QualifiedTypeName,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        schema_index
            .find_entity_type(qtype)
            .ok_or_else(|| Error::EntityTypeNotFound(qtype.into()))
            .and_then(|et| Self::compile_entity_type(qtype.into(), et, schema_index, stack))
            .map_err(Box::new)
            .map_err(|e| Error::EntityType(qtype.into(), e))
    }

    fn compile_entity_type<'a>(
        name: QualifiedName<'a>,
        schema_entity_type: &'a EntityType,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let stack = stack.new_frame().with_enitity_type(name);
        // Ensure that base entity type compiled if present.
        let (base, compiled) = if let Some(base_type) = &schema_entity_type.base_type {
            let compiled = Self::ensure_entity_type(base_type, schema_index, &stack)?;
            (Some(base_type.into()), compiled)
        } else {
            (None, Compiled::default())
        };
        let stack = stack.new_frame().merge(compiled);

        // Compile navigation and regular properties
        let (compiled, properties) = Self::compile_properties(
            &schema_entity_type.properties,
            schema_index,
            stack.new_frame(),
        )?;

        Ok(stack
            .merge(compiled)
            .merge(Compiled::new_entity_type(CompiledEntityType {
                name,
                base,
                key: schema_entity_type.key.as_ref(),
                properties,
                odata: CompiledOData::new(schema_entity_type),
            }))
            .done())
    }

    fn compile_properties<'a>(
        props: &'a [Property],
        schema_index: &SchemaIndex<'a>,
        stack: Stack<'a, '_>,
    ) -> Result<(Compiled<'a>, CompiledProperties<'a>), Error<'a>> {
        props
            .iter()
            .try_fold(
                (stack, CompiledProperties::default()),
                |(stack, mut p), sp| {
                    let stack = match &sp.attrs {
                        PropertyAttrs::StructuralProperty(v) => {
                            let compiled = Self::ensure_type(&v.ptype, schema_index, &stack)
                                .map_err(Box::new)
                                .map_err(|e| Error::Property(&sp.name, e))?;
                            p.properties.push(CompiledProperty {
                                name: &v.name,
                                ptype: (&v.ptype).into(),
                                odata: CompiledOData::new(v),
                            });
                            stack.merge(compiled)
                        }
                        PropertyAttrs::NavigationProperty(v) => {
                            let (ptype, compiled) = schema_index
                                // We are searching for deepest available child in tre
                                // hierarchy of types for singleton. So, we can parse most
                                // recent protocol versions.
                                .find_child_entity_type(v.ptype.qualified_type_name().into())
                                .and_then(|(qtype, et)| {
                                    if stack.contains_entity(qtype) {
                                        // Aready compiled entity
                                        Ok(Compiled::default())
                                    } else {
                                        Self::compile_entity_type(qtype, et, schema_index, &stack)
                                            .map_err(Box::new)
                                            .map_err(|e| Error::EntityType(qtype, e))
                                    }
                                    .map(|compiled| (qtype, compiled))
                                })
                                .map_err(Box::new)
                                .map_err(|e| Error::Property(&sp.name, e))?;
                            p.nav_properties.push(CompiledNavProperty {
                                name: &v.name,
                                ptype: match &v.ptype {
                                    TypeName::One(_) => CompiledPropertyType::One(ptype),
                                    TypeName::CollectionOf(_) => {
                                        CompiledPropertyType::CollectionOf(ptype)
                                    }
                                },
                                odata: CompiledOData::new(v),
                            });
                            stack.merge(compiled)
                        }
                    };
                    Ok((stack, p))
                },
            )
            .map(|(stack, p)| (stack.done(), p))
    }

    fn is_simple_type(qtype: &QualifiedTypeName) -> bool {
        qtype.inner().namespace.is_edm()
    }

    fn ensure_type<'a>(
        typename: &'a TypeName,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let qtype = match typename {
            TypeName::One(v) | TypeName::CollectionOf(v) => v,
        };
        if stack.contains_entity(qtype.into()) || Self::is_simple_type(qtype) {
            Ok(Compiled::default())
        } else {
            Self::compile_type(qtype, schema_index, stack)
        }
    }

    fn compile_type<'a>(
        qtype: &'a QualifiedTypeName,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        schema_index
            .find_type(qtype)
            .ok_or_else(|| Error::TypeNotFound(qtype.into()))
            .and_then(|t| match t {
                Type::TypeDefinition(td) => {
                    let underlying_type = (&td.underlying_type).into();
                    if Self::is_simple_type(&td.underlying_type) {
                        Ok(Compiled::new_type_definition(CompiledTypeDefinition {
                            name: qtype.into(),
                            underlying_type,
                        }))
                    } else {
                        Err(Error::TypeDefinitionOfNotPrimitiveType(underlying_type))
                    }
                }
                Type::EnumType(et) => {
                    let underlying_type = et.underlying_type.unwrap_or_default();
                    Ok(Compiled::new_enum_type(CompiledEnumType {
                        name: qtype.into(),
                        underlying_type,
                        members: et.members.iter().map(Into::into).collect(),
                        odata: CompiledOData::new(et),
                    }))
                }
                Type::ComplexType(ct) => {
                    let name = qtype.into();
                    // Ensure that base entity type compiled if present.
                    let (base, compiled) = if let Some(base_type) = &ct.base_type {
                        let compiled = Self::compile_type(base_type, schema_index, stack)?;
                        (Some(base_type.into()), compiled)
                    } else {
                        (None, Compiled::default())
                    };

                    let stack = stack.new_frame().merge(compiled);

                    let (compiled, properties) =
                        Self::compile_properties(&ct.properties, schema_index, stack.new_frame())?;

                    Ok(stack
                        .merge(compiled)
                        .merge(Compiled::new_complex_type(CompiledComplexType {
                            name,
                            base,
                            properties,
                            odata: CompiledOData::new(ct),
                        }))
                        .done())
                }
            })
            .map_err(Box::new)
            .map_err(|e| Error::Type(qtype.into(), e))
    }
}

pub trait PropertiesManipulation<'a> {
    #[must_use]
    fn map_properties<F>(self, f: F) -> Self
    where
        F: Fn(CompiledProperty<'a>) -> CompiledProperty<'a>;

    #[must_use]
    fn map_nav_properties<F>(self, f: F) -> Self
    where
        F: Fn(CompiledNavProperty<'a>) -> CompiledNavProperty<'a>;
}

pub trait MapType<'a> {
    #[must_use]
    fn map_type<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>;
}

pub trait MapBase<'a> {
    #[must_use]
    fn map_base<F>(self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>;
}

#[derive(Debug)]
pub struct CompiledEntityType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub key: Option<&'a Key>,
    pub properties: CompiledProperties<'a>,
    pub odata: CompiledOData<'a>,
}

impl<'a> PropertiesManipulation<'a> for CompiledEntityType<'a> {
    fn map_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(CompiledProperty<'a>) -> CompiledProperty<'a>,
    {
        self.properties.properties = self.properties.properties.into_iter().map(f).collect();
        self
    }

    fn map_nav_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(CompiledNavProperty<'a>) -> CompiledNavProperty<'a>,
    {
        self.properties.nav_properties =
            self.properties.nav_properties.into_iter().map(f).collect();
        self
    }
}

impl<'a> MapBase<'a> for CompiledEntityType<'a> {
    fn map_base<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.base = self.base.map(f);
        self
    }
}

#[derive(Debug)]
pub struct CompiledComplexType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub properties: CompiledProperties<'a>,
    pub odata: CompiledOData<'a>,
}

impl<'a> PropertiesManipulation<'a> for CompiledComplexType<'a> {
    fn map_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(CompiledProperty<'a>) -> CompiledProperty<'a>,
    {
        self.properties.properties = self.properties.properties.into_iter().map(f).collect();
        self
    }

    fn map_nav_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(CompiledNavProperty<'a>) -> CompiledNavProperty<'a>,
    {
        self.properties.nav_properties =
            self.properties.nav_properties.into_iter().map(f).collect();
        self
    }
}

impl<'a> MapBase<'a> for CompiledComplexType<'a> {
    fn map_base<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.base = self.base.map(f);
        self
    }
}

#[derive(Debug)]
pub enum CompiledPropertyType<'a> {
    One(QualifiedName<'a>),
    CollectionOf(QualifiedName<'a>),
}

impl<'a> CompiledPropertyType<'a> {
    #[must_use]
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        match self {
            Self::One(v) => Self::One(f(v)),
            Self::CollectionOf(v) => Self::CollectionOf(f(v)),
        }
    }
}

impl<'a> From<&'a TypeName> for CompiledPropertyType<'a> {
    fn from(v: &'a TypeName) -> Self {
        match v {
            TypeName::One(v) => Self::One(v.into()),
            TypeName::CollectionOf(v) => Self::CollectionOf(v.into()),
        }
    }
}

#[derive(Debug)]
pub struct CompiledProperty<'a> {
    pub name: &'a PropertyName,
    pub ptype: CompiledPropertyType<'a>,
    pub odata: CompiledOData<'a>,
}

impl<'a> MapType<'a> for CompiledProperty<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = self.ptype.map(f);
        self
    }
}

#[derive(Debug)]
pub struct CompiledNavProperty<'a> {
    pub name: &'a PropertyName,
    pub ptype: CompiledPropertyType<'a>,
    pub odata: CompiledOData<'a>,
}

impl<'a> MapType<'a> for CompiledNavProperty<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = self.ptype.map(f);
        self
    }
}

#[derive(Debug)]
pub struct CompiledSingleton<'a> {
    pub name: &'a SimpleIdentifier,
    pub stype: QualifiedName<'a>,
}

impl<'a> MapType<'a> for CompiledSingleton<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.stype = f(self.stype);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::edmx::Edmx;

    #[test]
    fn schema_test() {
        let schema = r#"<edmx:Edmx Version="4.0">
             <edmx:DataServices>
               <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Resource">
                 <EntityType Name="ItemOrCollection" Abstract="true"/>
                 <EntityType Name="Item" BaseType="Resource.ItemOrCollection" Abstract="true"/>
                 <EntityType Name="Resource" BaseType="Resource.Item" Abstract="true"/>
               </Schema>
               <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Resource.v1_0_0">
                 <EntityType Name="Resource" BaseType="Resource.Resource" Abstract="true">
                   <Key><PropertyRef Name="Id"/></Key>
                 </EntityType>
               </Schema>
               <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="ServiceRoot">
                 <EntityType Name="ServiceRoot" BaseType="Resource.v1_0_0.Resource" Abstract="true">
                   <Property Name="RedfishVersion" Type="Edm.String" Nullable="false">
                     <Annotation Term="OData.Description" String="The version of the Redfish service."/>
                   </Property>
                 </EntityType>
               </Schema>
               <Schema Namespace="Schema.v1_0_0">
                 <EntityContainer Name="ServiceContainer">
                   <Singleton Name="Service" Type="ServiceRoot.ServiceRoot"/>
                 </EntityContainer>
                 <EntityType Name="ServiceRoot" BaseType="ServiceRoot.ServiceRoot"/>
               </Schema>
             </edmx:DataServices>
           </edmx:Edmx>"#;
        let bundle = SchemaBundle {
            edmx_docs: vec![Edmx::parse(schema).unwrap()],
        };
        let compiled = bundle.compile(&["Service".parse().unwrap()]).unwrap();
        assert_eq!(compiled.root_singletons.len(), 1);
        let mut cur_type = &compiled.root_singletons.first().unwrap().stype;
        loop {
            let et = compiled.entity_types.get(cur_type).unwrap();
            cur_type = if let Some(t) = &et.base { t } else { break };
        }
        let qtype: QualifiedTypeName = "ServiceRoot.ServiceRoot".parse().unwrap();
        let et = compiled.entity_types.get(&(&qtype).into()).unwrap();
        assert_eq!(et.properties.properties.len(), 1);
        assert_eq!(
            et.properties.properties[0]
                .odata
                .description
                .as_ref()
                .unwrap()
                .inner(),
            &"The version of the Redfish service."
        );
    }
}
