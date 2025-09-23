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

/// Compiled redfish attrs
pub mod redfish;

/// Traits that are useful for compilation.
pub mod traits;

/// Compiled properties of `ComplexType` or `EntityType`
pub mod properties;

/// Compiled enum type
pub mod enum_type;

/// Compiled type definition
pub mod type_definition;

/// Compiled entity type
pub mod entity_type;

/// Compiled complex type
pub mod complex_type;

/// Compiled singleton
pub mod singleton;

use crate::compiler::odata::MustHaveId;
use crate::edmx::ActionName;
use crate::edmx::Edmx;
use crate::edmx::IsNullable;
use crate::edmx::ParameterName;
use crate::edmx::QualifiedTypeName;
use crate::edmx::action::Action;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::edmx::attribute_values::TypeName;
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
/// Reexport `Namespace` to the level of the compiler.
pub type Namespace<'a> = namespace::Namespace<'a>;
/// Reexport `CompiledOData` to the level of the compiler.
pub type OData<'a> = odata::OData<'a>;
/// Reexport `Properties` to the level of the compiler.
pub type Properties<'a> = properties::Properties<'a>;
/// Reexport `Property` to the level of the compiler.
pub type Property<'a> = properties::Property<'a>;
/// Reexport `NavProperty` to the level of the compiler.
pub type NavProperty<'a> = properties::NavProperty<'a>;
/// Reexport `PropertyType` to the level of the compiler.
pub type PropertyType<'a> = properties::PropertyType<'a>;
/// Reexport `EnumType` to the level of the compiler.
pub type EnumType<'a> = enum_type::EnumType<'a>;
/// Reexport `TypeDefinition` to the level of the compiler.
pub type TypeDefinition<'a> = type_definition::TypeDefinition<'a>;
/// Reexport `CompiledEntityType` to the level of the compiler.
pub type EntityType<'a> = entity_type::EntityType<'a>;
/// Reexport `ComplexType` to the level of the compiler.
pub type ComplexType<'a> = complex_type::ComplexType<'a>;
/// Reexport `CompiledComplexType` to the level of the compiler.
pub type Singleton<'a> = singleton::Singleton<'a>;

/// Reexport `MapBase` to the level of the compiler.
pub use traits::MapBase;
/// Reexport `MapType` to the level of the compiler.
pub use traits::MapType;
/// Reexport `PropertiesManipulation` to the level of the compiler.
pub use traits::PropertiesManipulation;

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
        let stack = self.edmx_docs.iter().try_fold(stack, |stack, edmx| {
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
        })?;
        // Compile actions for all extracted types
        self.edmx_docs
            .iter()
            .try_fold(stack, |stack, edmx| {
                let cstack = stack.new_frame();
                let compiled = edmx
                    .data_services
                    .schemas
                    .iter()
                    .try_fold(cstack, |stack, s| {
                        Self::compile_schema_actions(s, &schema_index, stack.new_frame())
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
                            Singleton::compile(s, schema_index, &stack).map(|v| stack.merge(v))
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

    fn compile_schema_actions<'a>(
        s: &'a Schema,
        schema_index: &SchemaIndex<'a>,
        stack: Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        s.actions
            .iter()
            .try_fold(stack, |stack, action| {
                let compiled = Self::compile_action(action, schema_index, &stack)
                    .map_err(Box::new)
                    .map_err(|e| Error::Action(&action.name, e))?;
                Ok(stack.merge(compiled))
            })
            .map_err(Box::new)
            .map_err(|e| Error::Schema(&s.namespace, e))
            .map(Stack::done)
    }

    fn compile_action<'a>(
        action: &'a Action,
        schema_index: &SchemaIndex<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        if !action.is_bound.into_inner() {
            return Err(Error::NotBoundAction);
        }
        let mut iter = action.parameters.iter();
        let binding_param = iter.next().ok_or(Error::NoBindingParameterForAction)?;
        let binding = binding_param.ptype.qualified_type_name().into();
        // If action is bound to not compiled type, just ignore it. We
        // will not have node to attach this action. Note: This may
        // not be correct for common CSDL schema but Redfish always
        // points to ComplexType (Actions).
        if !stack.contains_complex_type(binding) {
            return Ok(Compiled::default());
        }
        let stack = stack.new_frame();
        // Compile ReturnType if present
        let (compiled_rt, return_type) = action
            .return_type
            .as_ref()
            .map_or_else(
                || Ok((Compiled::default(), None)),
                |rt| {
                    ensure_type(&rt.rtype, schema_index, &stack)
                        .map(|compiled| (compiled, Some((&rt.rtype).into())))
                },
            )
            .map_err(Box::new)
            .map_err(Error::ActionReturnType)?;
        let stack = stack.merge(compiled_rt);
        // Compile other parameters except first one
        let (stack, parameters) =
            iter.try_fold((stack, Vec::new()), |(cstack, mut params), p| {
                // Sometime parameters refers to entity types. This is
                // different from complex types / entity types where
                // properties only points to complex / simple types
                // and navigation poprerties points to entity type
                // only. Example is AddResourceBlock in
                // ComputerSystem schema.
                let qtype_name = p.ptype.qualified_type_name();
                let (compiled, ptype) = if is_simple_type(qtype_name) {
                    Ok((
                        Compiled::default(),
                        CompiledParameterType::Type((&p.ptype).into()),
                    ))
                } else if schema_index.find_type(qtype_name).is_some() {
                    ensure_type(&p.ptype, schema_index, &cstack)
                        .map(|compiled| (compiled, CompiledParameterType::Type((&p.ptype).into())))
                } else {
                    EntityType::ensure(qtype_name, schema_index, &cstack).map(|compiled| {
                        (compiled, CompiledParameterType::Entity((&p.ptype).into()))
                    })
                }
                .map_err(Box::new)
                .map_err(|e| Error::ActionParameter(&p.name, e))?;
                params.push(CompiledParameter {
                    name: &p.name,
                    ptype,
                    is_nullable: p.nullable.unwrap_or(IsNullable::new(true)),
                });
                Ok((cstack.merge(compiled), params))
            })?;
        Ok(stack
            .merge(Compiled::new_action(CompiledAction {
                binding,
                name: &action.name,
                return_type,
                parameters,
            }))
            .done())
    }
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
    if stack.contains_entity(qtype.into()) || is_simple_type(qtype) {
        Ok(Compiled::default())
    } else {
        compile_type(qtype, schema_index, stack)
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
                if is_simple_type(&td.underlying_type) {
                    Ok(Compiled::new_type_definition(TypeDefinition {
                        name: qtype.into(),
                        underlying_type,
                    }))
                } else {
                    Err(Error::TypeDefinitionOfNotPrimitiveType(underlying_type))
                }
            }
            Type::EnumType(et) => {
                let underlying_type = et.underlying_type.unwrap_or_default();
                Ok(Compiled::new_enum_type(EnumType {
                    name: qtype.into(),
                    underlying_type,
                    members: et.members.iter().map(Into::into).collect(),
                    odata: OData::new(MustHaveId::new(false), et),
                }))
            }
            Type::ComplexType(ct) => {
                let name = qtype.into();
                // Ensure that base entity type compiled if present.
                let (base, compiled) = if let Some(base_type) = &ct.base_type {
                    let compiled = compile_type(base_type, schema_index, stack)?;
                    (Some(base_type.into()), compiled)
                } else {
                    (None, Compiled::default())
                };

                let stack = stack.new_frame().merge(compiled);

                let (compiled, properties) =
                    Properties::compile(&ct.properties, schema_index, stack.new_frame())?;

                Ok(stack
                    .merge(compiled)
                    .merge(Compiled::new_complex_type(ComplexType {
                        name,
                        base,
                        properties,
                        odata: OData::new(MustHaveId::new(false), ct),
                    }))
                    .done())
            }
        })
        .map_err(Box::new)
        .map_err(|e| Error::Type(qtype.into(), e))
}

/// Compiled parameter of the action.
#[derive(Debug)]
pub struct CompiledParameter<'a> {
    /// Name of the parameter.
    pub name: &'a ParameterName,
    /// Type of the parameter. Can be either entity reference or some
    /// specific type.
    pub ptype: CompiledParameterType<'a>,
    /// Flag that parameter is nullable.
    pub is_nullable: IsNullable,
}

/// Type of the parameter. Note we reuse `CompiledPropertyType`, it
/// maybe not exact and may be change in future.
#[derive(Debug)]
pub enum CompiledParameterType<'a> {
    Entity(PropertyType<'a>),
    Type(PropertyType<'a>),
}

impl<'a> CompiledParameterType<'a> {
    fn map<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        match self {
            Self::Entity(v) => Self::Entity(v.map(f)),
            Self::Type(v) => Self::Type(v.map(f)),
        }
    }
}

impl<'a> MapType<'a> for CompiledParameter<'a> {
    fn map_type<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        Self {
            name: self.name,
            ptype: self.ptype.map(f),
            is_nullable: self.is_nullable,
        }
    }
}

/// Compuled parameter of the action.
#[derive(Debug)]
pub struct CompiledAction<'a> {
    /// Bound type.
    pub binding: QualifiedName<'a>,
    /// Name of the parameter.
    pub name: &'a ActionName,
    /// Type of the return value. Note we reuse
    /// `CompiledPropertyType`, it maybe not exact and may be change
    /// in future.
    pub return_type: Option<PropertyType<'a>>,
    /// Type of the parameter. Note we reuse `CompiledPropertyType`, it
    /// maybe not exact and may be change in future.
    pub parameters: Vec<CompiledParameter<'a>>,
}

impl<'a> MapType<'a> for CompiledAction<'a> {
    fn map_type<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        Self {
            name: self.name,
            binding: f(self.binding),
            return_type: self.return_type.map(|rt| rt.map(&f)),
            parameters: self
                .parameters
                .into_iter()
                .map(|p| p.map_type(&f))
                .collect(),
        }
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
