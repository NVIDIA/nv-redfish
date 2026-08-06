// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Schema queries for projection compilers.

use std::collections::HashMap;
use std::iter;

use crate::compiler::Compiled;
use crate::compiler::Config;
use crate::compiler::EntityTypeFilter;
use crate::compiler::Error;
use crate::compiler::Properties;
use crate::compiler::Property;
use crate::compiler::QualifiedName;
use crate::compiler::SchemaBundle;
use crate::compiler::TypeClass;
use crate::edmx::EnumMemberName;
use crate::OneOrCollection;

/// One resolved structural property.
#[derive(Debug)]
pub struct ResolvedProperty<'a> {
    pub type_name: QualifiedName<'a>,
    pub class: TypeClass,
    pub nullable: bool,
    pub collection: bool,
    pub enum_members: Option<Vec<&'a EnumMemberName>>,
}

/// A queryable view over a compiled bundle.
pub struct SchemaQuery<'a> {
    compiled: Compiled<'a>,
    entities: HashMap<&'a str, QualifiedName<'a>>,
}

impl<'a> SchemaQuery<'a> {
    /// Compiles the whole bundle and indexes its entity types.
    ///
    /// # Errors
    ///
    /// Returns a compile error if any type cannot be resolved.
    pub fn build(bundle: &'a SchemaBundle) -> Result<Self, Error<'a>> {
        let config = Config {
            entity_type_filter: EntityTypeFilter::new_restrictive(Vec::new()),
            ..Config::default()
        };
        let compiled = bundle.compile_all(config)?;

        let mut entities = HashMap::new();
        for qname in compiled.entity_types.keys() {
            let name = qname.name.inner().as_str();
            let replace = entities.get(name).is_none_or(|current| {
                chain_length(&compiled, *qname) > chain_length(&compiled, *current)
            });
            if replace {
                entities.insert(name, *qname);
            }
        }
        Ok(Self { compiled, entities })
    }

    /// Whether the bundle declares an entity type of this name.
    #[must_use]
    pub fn has_entity(&self, name: &str) -> bool {
        self.entities.contains_key(name)
    }

    /// Resolves a dotted property path against the named entity type,
    /// descending through complex types and walking base chains.
    #[must_use]
    pub fn resolve(&self, entity: &str, path: &str) -> Option<ResolvedProperty<'a>> {
        let mut current = TypeRef::Entity(*self.entities.get(entity)?);
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            let property = self.property_of(current, segment)?;
            let (info, type_name) = match &property.ptype {
                OneOrCollection::One(inner) => (inner, false),
                OneOrCollection::Collection(inner) => (inner, true),
            };
            let (class, collection) = (info.0.class, type_name);
            let qname = info.1;
            if segments.peek().is_none() {
                let enum_members = match class {
                    TypeClass::EnumType => self.compiled.enum_types.get(&qname).map(|declared| {
                        declared.members.iter().map(|member| member.name).collect()
                    }),
                    _ => None,
                };
                return Some(ResolvedProperty {
                    type_name: qname,
                    class,
                    nullable: property.nullable.into_inner(),
                    collection,
                    enum_members,
                });
            }
            if class != TypeClass::ComplexType {
                return None;
            }
            current = TypeRef::Complex(qname);
        }
        None
    }

    /// The named structural property, searched up the base chain.
    fn property_of(&self, mut tref: TypeRef<'a>, name: &str) -> Option<&Property<'a>> {
        loop {
            let (properties, base) = self.declaration(tref)?;
            if let Some(property) = properties
                .properties
                .iter()
                .find(|property| property.name.inner().inner() == name)
            {
                return Some(property);
            }
            tref = match tref {
                TypeRef::Entity(_) => TypeRef::Entity(base?),
                TypeRef::Complex(_) => TypeRef::Complex(base?),
            };
        }
    }

    fn declaration(
        &self,
        tref: TypeRef<'a>,
    ) -> Option<(&Properties<'a>, Option<QualifiedName<'a>>)> {
        match tref {
            TypeRef::Entity(qname) => self
                .compiled
                .entity_types
                .get(&qname)
                .map(|entity| (&entity.properties, entity.base)),
            TypeRef::Complex(qname) => self
                .compiled
                .complex_types
                .get(&qname)
                .map(|complex| (&complex.properties, complex.base)),
        }
    }
}

#[derive(Clone, Copy)]
enum TypeRef<'a> {
    Entity(QualifiedName<'a>),
    Complex(QualifiedName<'a>),
}

fn chain_length<'a>(compiled: &Compiled<'a>, start: QualifiedName<'a>) -> usize {
    iter::successors(Some(start), |&qname| {
        compiled.entity_types.get(&qname)?.base
    })
    .count()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::edmx::Edmx;

    fn bundle() -> SchemaBundle {
        // `compile_all` unconditionally compiles the Redfish framework
        // types, so the fixture carries minimal Resource and Settings
        // declarations beside the types under test.
        let schema = r#"<edmx:Edmx Version="4.0">
          <edmx:DataServices>
            <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Resource">
              <EntityType Name="Resource" Abstract="true">
                <Property Name="Id" Type="Edm.String" Nullable="false"/>
              </EntityType>
              <EntityType Name="ResourceCollection" Abstract="true"/>
              <ComplexType Name="Status">
                <Property Name="Health" Type="Resource.Health" Nullable="true"/>
              </ComplexType>
              <EnumType Name="Health">
                <Member Name="OK"/>
                <Member Name="Warning"/>
              </EnumType>
            </Schema>
            <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Settings">
              <ComplexType Name="Settings"/>
              <ComplexType Name="PreferredApplyTime"/>
            </Schema>
            <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Widget">
              <EntityType Name="Widget" Abstract="true" BaseType="Resource.Resource"/>
            </Schema>
            <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Widget.v1_0_0">
              <EntityType Name="Widget" BaseType="Widget.Widget">
                <Property Name="Reading" Type="Edm.Decimal" Nullable="true"/>
                <Property Name="Status" Type="Resource.Status" Nullable="false"/>
              </EntityType>
            </Schema>
            <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Widget.v1_1_0">
              <EntityType Name="Widget" BaseType="Widget.v1_0_0.Widget">
                <Property Name="Labels" Type="Collection(Edm.String)" Nullable="false"/>
              </EntityType>
            </Schema>
          </edmx:DataServices>
        </edmx:Edmx>"#;
        SchemaBundle {
            edmx_docs: vec![Edmx::parse(schema).expect("query test schema must be valid")],
            root_set_threshold: None,
        }
    }

    #[test]
    fn resolves_across_versions_bases_and_complex_types() {
        let bundle = bundle();
        let query = SchemaQuery::build(&bundle).expect("bundle compiles");

        assert!(query.has_entity("Widget"));
        assert!(!query.has_entity("Gadget"));

        // Declared in v1_0_0, visible from the most derived fold.
        let reading = query
            .resolve("Widget", "Reading")
            .expect("Reading resolves");
        assert_eq!(reading.type_name.to_string(), "Edm.Decimal");
        assert_eq!(reading.class, TypeClass::SimpleType);
        assert!(reading.nullable);
        assert!(!reading.collection);

        // Declared on the abstract base of the base.
        let id = query.resolve("Widget", "Id").expect("Id resolves");
        assert!(!id.nullable);

        // Added in v1_1_0: the index picked the most derived version.
        let labels = query.resolve("Widget", "Labels").expect("Labels resolve");
        assert!(labels.collection);

        // Through a complex type to an enum, members included.
        let health = query
            .resolve("Widget", "Status.Health")
            .expect("Status.Health resolves");
        assert_eq!(health.class, TypeClass::EnumType);
        let members: Vec<String> = health
            .enum_members
            .expect("an enum has members")
            .iter()
            .map(|member| member.inner().to_string())
            .collect();
        assert_eq!(members, ["OK", "Warning"]);

        // Unknown paths and descent through scalars resolve to nothing.
        assert!(query.resolve("Widget", "Readng").is_none());
        assert!(query.resolve("Widget", "Reading.Deeper").is_none());
    }
}
