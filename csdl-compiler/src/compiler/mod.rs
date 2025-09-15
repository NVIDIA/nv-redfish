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

/// Index of schemas.
pub mod schema_index;

/// Compilation stack.
pub mod stack;

/// Error diagnostics
pub mod error;

/// Qualified name
pub mod qualified_name;

use crate::edmx::Edmx;
use crate::edmx::PropertyName;
use crate::edmx::QualifiedTypeName;
use crate::edmx::Singleton;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::edmx::attribute_values::TypeName;
use crate::edmx::entity_type::EntityType;
use crate::edmx::enum_type::EnumUnderlyingType;
use crate::edmx::property::Property;
use crate::edmx::property::PropertyAttrs;
use crate::edmx::schema::Schema;
use crate::edmx::schema::Type;
use crate::odata::annotations::DescriptionRef;
use crate::odata::annotations::LongDescriptionRef;
use crate::odata::annotations::ODataAnnotations as _;
use schema_index::SchemaIndex;
use stack::Stack;
use std::collections::HashMap;

/// Rexport `Error` to the level of compiler.
pub type Error<'a> = error::Error<'a>;
/// Rexport `QualifiedName` to the level of compiler.
pub type QualifiedName<'a> = qualified_name::QualifiedName<'a>;

#[derive(Default)]
pub struct SchemaBundle {
    pub edmx_docs: Vec<Edmx>,
}

impl SchemaBundle {
    /// Compile multiple schema, resolving all type dependencies.
    ///
    ///  # Errors
    ///
    /// Returns compile error if any type cannot be resolved.
    pub fn compile(&self) -> Result<Compiled<'_>, Error> {
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
                        Self::compile_schema(s, &schema_index, stack.new_frame())
                            .map(|v| stack.merge(v))
                    })?
                    .done();
                Ok(stack.merge(compiled))
            })
            .map(Stack::done)
    }

    fn compile_schema<'a>(
        s: &'a Schema,
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
                        Self::compile_singleton(s, schema_index, &stack).map(|v| stack.merge(v))
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
        let (stack, nav_properties, properties) =
            Self::compile_properties(&schema_entity_type.properties, schema_index, stack)?;

        Ok(stack
            .merge(Compiled::new_entity_type(CompiledEntityType {
                name,
                base,
                properties,
                nav_properties,
                description: schema_entity_type.odata_description(),
                long_description: schema_entity_type.odata_long_description(),
            }))
            .done())
    }

    fn compile_properties<'a, 'stack>(
        props: &'a [Property],
        schema_index: &SchemaIndex<'a>,
        stack: Stack<'a, 'stack>,
    ) -> Result<
        (
            Stack<'a, 'stack>,
            Vec<CompiledNavProperty<'a>>,
            Vec<CompiledProperty<'a>>,
        ),
        Error<'a>,
    > {
        props.iter().try_fold(
            (stack, Vec::new(), Vec::new()),
            |(stack, mut np, mut p), sp| {
                let stack = match &sp.attrs {
                    PropertyAttrs::StructuralProperty(v) => {
                        let compiled = Self::ensure_type(&v.ptype, schema_index, &stack)
                            .map_err(Box::new)
                            .map_err(|e| Error::Property(&sp.name, e))?;
                        p.push(CompiledProperty {
                            name: &v.name,
                            ptype: (&v.ptype).into(),
                            description: v.odata_description(),
                            long_description: v.odata_long_description(),
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
                        np.push(CompiledNavProperty {
                            name: &v.name,
                            ptype: match &v.ptype {
                                TypeName::One(_) => CompiledPropertyType::One(ptype),
                                TypeName::CollectionOf(_) => {
                                    CompiledPropertyType::CollectionOf(ptype)
                                }
                            },
                            description: v.odata_description(),
                            long_description: v.odata_long_description(),
                        });
                        stack.merge(compiled)
                    }
                };
                Ok((stack, np, p))
            },
        )
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

                    let (stack, nav_properties, properties) =
                        Self::compile_properties(&ct.properties, schema_index, stack.new_frame())?;

                    Ok(stack
                        .merge(Compiled::new_complex_type(CompiledComplexType {
                            name,
                            base,
                            properties,
                            nav_properties,
                            description: ct.odata_description(),
                            long_description: ct.odata_long_description(),
                        }))
                        .done())
                }
            })
            .map_err(Box::new)
            .map_err(|e| Error::Type(qtype.into(), e))
    }
}

#[derive(Default, Debug)]
pub struct Compiled<'a> {
    pub complex_types: HashMap<QualifiedName<'a>, CompiledComplexType<'a>>,
    pub entity_types: HashMap<QualifiedName<'a>, CompiledEntityType<'a>>,
    pub types: HashMap<QualifiedName<'a>, CompiledType<'a>>,
    pub root_singletons: Vec<CompiledSingleton<'a>>,
}

impl<'a> Compiled<'a> {
    #[must_use]
    pub fn new_entity_type(v: CompiledEntityType<'a>) -> Self {
        Self {
            entity_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_complex_type(v: CompiledComplexType<'a>) -> Self {
        Self {
            complex_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_singleton(v: CompiledSingleton<'a>) -> Self {
        Self {
            root_singletons: vec![v],
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_type_definition(v: CompiledTypeDefinition<'a>) -> Self {
        Self {
            types: vec![(v.name, CompiledType::TypeDefinition(v))]
                .into_iter()
                .collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_enum_type(v: CompiledEnumType<'a>) -> Self {
        Self {
            types: vec![(v.name, CompiledType::EnumType(v))]
                .into_iter()
                .collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.complex_types.extend(other.complex_types);
        self.types.extend(other.types);
        self.entity_types.extend(other.entity_types);
        self.root_singletons.extend(other.root_singletons);
        self
    }
}

#[derive(Debug)]
pub enum CompiledType<'a> {
    TypeDefinition(CompiledTypeDefinition<'a>),
    EnumType(CompiledEnumType<'a>),
    ComplexType(CompiledComplexType<'a>),
}

#[derive(Debug)]
pub struct CompiledTypeDefinition<'a> {
    pub name: QualifiedName<'a>,
    pub underlying_type: QualifiedName<'a>,
}

#[derive(Debug)]
pub struct CompiledEnumType<'a> {
    pub name: QualifiedName<'a>,
    pub underlying_type: EnumUnderlyingType,
}

#[derive(Debug)]
pub struct CompiledEntityType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub properties: Vec<CompiledProperty<'a>>,
    pub nav_properties: Vec<CompiledNavProperty<'a>>,
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

#[derive(Debug)]
pub struct CompiledComplexType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub properties: Vec<CompiledProperty<'a>>,
    pub nav_properties: Vec<CompiledNavProperty<'a>>,
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

#[derive(Debug)]
pub enum CompiledPropertyType<'a> {
    One(QualifiedName<'a>),
    CollectionOf(QualifiedName<'a>),
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
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

#[derive(Debug)]
pub struct CompiledNavProperty<'a> {
    pub name: &'a PropertyName,
    pub ptype: CompiledPropertyType<'a>,
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

#[derive(Debug)]
pub struct CompiledSingleton<'a> {
    pub name: &'a SimpleIdentifier,
    pub stype: QualifiedName<'a>,
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
        let compiled = bundle.compile().unwrap();
        assert_eq!(compiled.root_singletons.len(), 1);
        let mut cur_type = &compiled.root_singletons.first().unwrap().stype;
        loop {
            let et = compiled.entity_types.get(cur_type).unwrap();
            cur_type = if let Some(t) = &et.base { t } else { break };
        }
        let qtype: QualifiedTypeName = "ServiceRoot.ServiceRoot".parse().unwrap();
        let et = compiled.entity_types.get(&(&qtype).into()).unwrap();
        assert_eq!(et.properties.len(), 1);
        assert_eq!(
            et.properties[0].description.as_ref().unwrap().inner(),
            &"The version of the Redfish service."
        );
    }
}
