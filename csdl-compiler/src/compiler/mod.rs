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

//! Schema compiler pipeline
//!
//! This module turns a set of EDMX schemas into an intermediate
//! representation (`Compiled`) that is later consumed by the Rust
//! generator. The flow is intentionally simple and predictable:
//!
//! 1) Index
//!    - Build a `SchemaIndex` across all EDMX documents to resolve
//!      names and follow inheritance chains (entity/complex types).
//!
//! 2) Root set
//!    - Choose what to compile: either a root set derived from service
//!      singletons (`compile`) or all entity/complex types
//!      (`compile_all`). The `Config`/`EntityTypeFilter` can narrow
//!      which navigation targets are pulled in.
//!
//! 3) Traverse and compile
//!    - Walk entity and complex types, compiling structural and
//!      navigation properties. Navigation properties resolve to the
//!      most specific descendant type that adds properties, allowing
//!      newer protocol versions to be targeted.
//!    - A `Stack` tracks frames and prevents cycles when types refer to
//!      each other via navigation properties.
//!    - `OData` and Redfish-specific annotations are captured alongside
//!      types for later codegen (permissions, insert/update/delete,
//!      required flags, etc.).
//!
//! 4) Actions
//!    - Compile bound actions, their parameters and return types, and
//!      attach them to the binding type in `Compiled`.
//!
//! Output
//! - The result is a `Compiled` aggregate containing maps of entity
//!   types, complex types, enums, type definitions, and actions. It is
//!   designed to be stable, readable, and straightforward for the
//!   generator to consume.

#![deny(missing_docs)]

/// Compiled action.
pub mod action;
/// Compiled schema bundle.
pub mod compiled;
/// Compiled complex type.
pub mod complex_type;
/// Compilation context.
pub mod context;
/// Compiled entity type.
pub mod entity_type;
/// Compiled enum type.
pub mod enum_type;
/// Error diagnostics.
pub mod error;
/// Compiled namespace.
pub mod namespace;
/// Compiled OData.
pub mod odata;
/// Compiled action parameter.
pub mod parameter;
/// Compiled properties of `ComplexType` or `EntityType`.
pub mod properties;
/// Qualified (namespace + name) type identifier.
pub mod qualified_name;
/// Compiled Redfish-specific attributes.
pub mod redfish;
/// Index over parsed schemas.
pub mod schema_index;
/// Compilation stack.
pub mod stack;
/// Traits useful during compilation.
pub mod traits;
/// Compiled type definition.
pub mod type_definition;

// Type re-exports
#[doc(inline)]
pub use action::Action;
#[doc(inline)]
pub use compiled::ActionsMap;
#[doc(inline)]
pub use compiled::Compiled;
#[doc(inline)]
pub use compiled::IsCreatable;
#[doc(inline)]
pub use compiled::TypeActions;
#[doc(inline)]
pub use complex_type::ComplexType;
#[doc(inline)]
pub use context::Config;
#[doc(inline)]
pub use context::Context;
#[doc(inline)]
pub use context::EntityTypeFilter;
#[doc(inline)]
pub use context::EntityTypeFilterPattern;
#[doc(inline)]
pub use entity_type::EntityType;
#[doc(inline)]
pub use enum_type::EnumType;
#[doc(inline)]
pub use error::Error;
#[doc(inline)]
pub use namespace::Namespace;
#[doc(inline)]
pub use odata::OData;
#[doc(inline)]
pub use parameter::Parameter;
#[doc(inline)]
pub use parameter::ParameterType;
#[doc(inline)]
pub use properties::NavProperty;
#[doc(inline)]
pub use properties::NavPropertyExpandable;
#[doc(inline)]
pub use properties::NavPropertyType;
#[doc(inline)]
pub use properties::Properties;
#[doc(inline)]
pub use properties::Property;
#[doc(inline)]
pub use properties::PropertyType;
#[doc(inline)]
pub use properties::TypeInfo;
#[doc(inline)]
pub use qualified_name::QualifiedName;
#[doc(inline)]
pub use redfish::Redfish;
#[doc(inline)]
pub use type_definition::TypeDefinition;

// Trait re-exports
#[doc(inline)]
pub use traits::MapBase;
#[doc(inline)]
pub use traits::MapType;
#[doc(inline)]
pub use traits::PropertiesManipulation;

use crate::compiler::odata::MustHaveId;
use crate::edmx::Edmx;
use crate::edmx::Schema;
use crate::edmx::SimpleIdentifier;
use crate::edmx::Type;
use schema_index::SchemaIndex;
use stack::Stack;

/// Type class for property attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeClass {
    /// Simple type like `Edm.String`, `Edm.Int64` etc.
    SimpleType,
    /// Enumeration type.
    EnumType,
    /// Type definition.
    TypeDefinition,
    /// Complex type.
    ComplexType,
}

/// Collection of EDMX documents compiled together to produce code.
#[derive(Default)]
pub struct SchemaBundle {
    /// Parsed and validated EDMX documents.
    pub edmx_docs: Vec<Edmx>,
    /// If set, defines how many documents belong to the "root set"
    /// (used by `compile_all`).
    pub root_set_threshold: Option<usize>,
}

/// Set of types that need to be compiled.
#[derive(Debug)]
pub struct RootSet<'a> {
    entity_types: Vec<QualifiedName<'a>>,
    complex_types: Vec<QualifiedName<'a>>,
}

impl SchemaBundle {
    /// Compile multiple schemas, resolving all type dependencies.
    ///
    /// The root set is defined by the specified singletons.
    ///
    /// # Errors
    ///
    /// Returns a compile error if any type cannot be resolved.
    pub fn compile(
        &self,
        singletons: &[SimpleIdentifier],
        root_patterns: &EntityTypeFilter,
        config: Config,
    ) -> Result<Compiled<'_>, Error<'_>> {
        let schema_index = SchemaIndex::build(&self.edmx_docs);
        let root_set = self.root_set_from_singletons(&schema_index, singletons, root_patterns)?;
        let ctx = Context {
            schema_index,
            config,
            root_set_entities: root_set.entity_types.iter().copied().collect(),
        };
        self.compile_root_set(&root_set, &ctx)
    }

    /// Compile multiple schemas, resolving all type dependencies.
    ///
    /// The root set includes all entity and complex types.
    ///
    /// # Errors
    ///
    /// Returns a compile error if any type cannot be resolved.
    pub fn compile_all(&self, config: Config) -> Result<Compiled<'_>, Error<'_>> {
        let root_set = self.root_set_all();
        let ctx = Context {
            schema_index: SchemaIndex::build(&self.edmx_docs),
            config,
            root_set_entities: root_set.entity_types.iter().copied().collect(),
        };
        self.compile_root_set(&root_set, &ctx)
    }

    fn root_set_from_singletons<'a>(
        &'a self,
        schema_index: &SchemaIndex<'a>,
        singletons: &[SimpleIdentifier],
        root_patterns: &EntityTypeFilter,
    ) -> Result<RootSet<'a>, Error<'a>> {
        // Iterate through all singletons located in
        // EDMX documents → schemas → entity containers.
        //
        // If a singleton matches the requested set, collect its most recent
        // descendant type into the root set.
        let entity_types = self
            .edmx_docs
            .iter()
            .flat_map(|edmx| {
                edmx.data_services.schemas.iter().flat_map(|s| {
                    s.entity_container
                        .as_ref()
                        .map_or(Vec::new(), |entity_container| {
                            entity_container
                                .singletons
                                .iter()
                                .filter_map(|singleton| {
                                    if singletons.contains(&singleton.name) {
                                        Some(
                                            schema_index
                                                .find_child_entity_type((&singleton.stype).into())
                                                .map(|(qname, _)| qname),
                                        )
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                        })
                })
            })
            .chain(self.edmx_docs.iter().flat_map(|edmx| {
                edmx.data_services
                    .schemas
                    .iter()
                    .flat_map(|s| {
                        s.entity_types
                            .values()
                            .filter_map(|t| {
                                let name = QualifiedName::new(&s.namespace, t.name.inner());
                                if root_patterns.matches(&name) {
                                    Some(Ok(name))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            }))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RootSet {
            entity_types,
            complex_types: Vec::new(),
        })
    }

    fn root_set_all(&self) -> RootSet<'_> {
        let entity_types = self
            .edmx_docs
            .iter()
            .take(self.root_set_threshold.unwrap_or(self.edmx_docs.len()))
            .flat_map(|edmx| {
                edmx.data_services
                    .schemas
                    .iter()
                    .flat_map(|s| {
                        s.entity_types
                            .values()
                            .map(|t| QualifiedName::new(&s.namespace, t.name.inner()))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let complex_types = self
            .edmx_docs
            .iter()
            .take(self.root_set_threshold.unwrap_or(self.edmx_docs.len()))
            .flat_map(|edmx| {
                edmx.data_services
                    .schemas
                    .iter()
                    .flat_map(|s| {
                        s.types
                            .values()
                            .filter_map(|t| {
                                if let Type::ComplexType(t) = t {
                                    Some(QualifiedName::new(&s.namespace, t.name.inner()))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        RootSet {
            entity_types,
            complex_types,
        }
    }

    fn compile_root_set<'a>(
        &'a self,
        root_set: &RootSet<'a>,
        ctx: &Context<'a>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let stack = Stack::default();
        let stack = root_set
            .entity_types
            .iter()
            .try_fold(stack, |cstack, qname| {
                EntityType::ensure(*qname, ctx, &cstack).map(|compiled| cstack.merge(compiled))
            })?;
        let stack = root_set.complex_types.iter().try_fold(stack, |cstack, t| {
            ensure_type(*t, ctx, &cstack).map(|(compiled, _)| cstack.merge(compiled))
        })?;
        // Compile type for @Redfish.Settings
        let (name, _) = ctx.schema_index.redfish_settings_type()?;
        let (compiled, _) = ensure_type(name, ctx, &stack)?;
        let stack = stack.merge(compiled);
        // Compile type for @Redfish.SettingsApplyTime
        let (name, _) = ctx
            .schema_index
            .redfish_settings_preferred_apply_time_type()?;
        let (compiled, _) = ensure_type(name, ctx, &stack)?;
        let stack = stack.merge(compiled);
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
                        Self::compile_schema_actions(s, ctx, stack.new_frame())
                            .map(|v| stack.merge(v))
                    })?
                    .done();
                Ok(stack.merge(compiled))
            })
            .map(Stack::done)
    }

    fn compile_schema_actions<'a>(
        s: &'a Schema,
        ctx: &Context<'a>,
        stack: Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        s.actions
            .iter()
            .try_fold(stack, |stack, action| {
                let compiled = action::compile_action(action, ctx, &stack)
                    .map_err(Box::new)
                    .map_err(|e| Error::Action(&action.name, e))?;
                Ok(stack.merge(compiled))
            })
            .map_err(Box::new)
            .map_err(|e| Error::Schema(&s.namespace, e))
            .map(Stack::done)
    }
}

fn is_simple_type(qtype: QualifiedName<'_>) -> bool {
    qtype.namespace.is_edm()
}

fn ensure_type<'a>(
    qtype: QualifiedName<'a>,
    ctx: &Context<'a>,
    stack: &Stack<'a, '_>,
) -> Result<(Compiled<'a>, TypeInfo), Error<'a>> {
    if is_simple_type(qtype) {
        Ok((Compiled::default(), TypeInfo::simple_type()))
    } else if let Some(info) = stack.complex_type_info(qtype) {
        Ok((Compiled::default(), info))
    } else if stack.contains_type_definition(qtype) {
        Ok((Compiled::default(), TypeInfo::type_definition()))
    } else if stack.contains_enum_type(qtype) {
        Ok((Compiled::default(), TypeInfo::enum_type()))
    } else {
        compile_type(qtype, ctx, stack)
    }
}

fn compile_type<'a>(
    qtype: QualifiedName<'a>,
    ctx: &Context<'a>,
    stack: &Stack<'a, '_>,
) -> Result<(Compiled<'a>, TypeInfo), Error<'a>> {
    ctx.schema_index
        .find_type(qtype)
        .ok_or(Error::TypeNotFound(qtype))
        .and_then(|t| match t {
            Type::TypeDefinition(td) => type_definition::compile(qtype, td),
            Type::EnumType(et) => Ok(enum_type::compile(qtype, et)),
            Type::ComplexType(ct) => complex_type::compile(qtype, ct, ctx, stack),
        })
        .map_err(Box::new)
        .map_err(|e| Error::Type(qtype, e))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::edmx::Edmx;
    use crate::edmx::QualifiedTypeName;

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
               <Schema Namespace="Settings">
                 <ComplexType Name="Settings"/>
                 <ComplexType Name="PreferredApplyTime"/>
               </Schema>
             </edmx:DataServices>
           </edmx:Edmx>"#;
        let bundle = SchemaBundle {
            edmx_docs: vec![Edmx::parse(schema).unwrap()],
            root_set_threshold: None,
        };
        let compiled = bundle
            .compile(
                &["Service".parse().unwrap()],
                &EntityTypeFilter::new_restrictive(vec![]),
                Config::default(),
            )
            .unwrap();
        let qtypename: QualifiedTypeName = "ServiceRoot.ServiceRoot".parse().unwrap();
        let root_type: QualifiedName<'_> = (&qtypename).into();
        let mut cur_type = &root_type;
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
