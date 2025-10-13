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

use crate::compiler::Error;
use crate::compiler::Namespace;
use crate::compiler::QualifiedName;
use crate::edmx::ComplexType;
use crate::edmx::Edmx;
use crate::edmx::EntityType;
use crate::edmx::Namespace as EdmxNamespace;
use crate::edmx::Schema;
use crate::edmx::SimpleIdentifier;
use crate::edmx::Type;
use std::collections::HashMap;
use std::convert::identity;

/// Indexing of schema across different documents
pub struct SchemaIndex<'a> {
    index: HashMap<Namespace<'a>, &'a Schema>,
    /// Mapping from base types to all inherited types. This index is
    /// built for complex and entity types.
    child_map: HashMap<QualifiedName<'a>, Vec<QualifiedName<'a>>>,
}

impl<'a> SchemaIndex<'a> {
    /// Build index from docs.
    #[must_use]
    pub fn build(edmx_docs: &'a [Edmx]) -> Self {
        Self {
            index: edmx_docs
                .iter()
                .flat_map(|v| {
                    v.data_services
                        .schemas
                        .iter()
                        .map(|s| (Namespace::new(&s.namespace), s))
                })
                .collect(),
            child_map: edmx_docs.iter().fold(HashMap::new(), |map, doc| {
                doc.data_services.schemas.iter().fold(map, |map, s| {
                    let entity_types = s
                        .entity_types
                        .values()
                        .filter_map(|et| et.base_type.as_ref().map(|base| (&et.name, base)));
                    let complex_types = s.types.values().filter_map(|t| {
                        if let Type::ComplexType(ct) = &t {
                            ct.base_type.as_ref().map(|base| (&ct.name, base))
                        } else {
                            None
                        }
                    });
                    entity_types
                        .chain(complex_types)
                        .fold(map, |mut map, (name, base)| {
                            let qname = QualifiedName::new(&s.namespace, name.inner());
                            let base_type: QualifiedName = base.into();
                            map.entry(base_type)
                                .and_modify(|e| e.push(qname))
                                .or_insert_with(|| vec![qname]);
                            map
                        })
                })
            }),
        }
    }

    /// Find schema by namespace.
    #[must_use]
    pub fn get(&self, ns: &Namespace<'_>) -> Option<&'a Schema> {
        self.index.get(ns).map(|v| &**v)
    }

    /// Find entity type by type name
    #[must_use]
    pub fn find_entity_type(&self, qtype: QualifiedName<'_>) -> Option<&'a EntityType> {
        self.get(&qtype.namespace)
            .and_then(|ns| ns.entity_types.get(qtype.name))
    }

    /// Find most specific entity type child.
    ///
    /// # Errors
    ///
    /// Returns error if entity type is not found.
    pub fn find_child_entity_type(
        &self,
        qtype: QualifiedName<'a>,
    ) -> Result<(QualifiedName<'a>, &'a EntityType), Error<'a>> {
        let qtype = self.find_child_type(qtype);
        self.get(&qtype.namespace)
            .and_then(|ns| ns.entity_types.get(qtype.name))
            .ok_or(Error::EntityTypeNotFound(qtype))
            .map(|v| (qtype, v))
    }

    /// Find most specific complex type child.
    ///
    /// # Errors
    ///
    /// Returns error if complex type is not found.
    pub fn find_child_complex_type(
        &self,
        qtype: QualifiedName<'a>,
    ) -> Result<(QualifiedName<'a>, &'a ComplexType), Error<'a>> {
        let qtype = self.find_child_type(qtype);
        self.get(&qtype.namespace)
            .and_then(|ns| ns.types.get(qtype.name))
            .and_then(|t| {
                if let Type::ComplexType(ct) = t {
                    Some(ct)
                } else {
                    None
                }
            })
            .ok_or(Error::ComplexTypeNotFound(qtype))
            .map(|v| (qtype, v))
    }

    /// Find type by type name
    #[must_use]
    pub fn find_type(&self, qtype: QualifiedName<'_>) -> Option<&'a Type> {
        self.get(&qtype.namespace)
            .and_then(|ns| ns.types.get(qtype.name))
    }

    /// Find child type by typename. For complex / entity types it
    /// returns most distant unique descendant. For other types just
    /// returns parameter.
    #[must_use]
    pub fn find_child_type(&self, mut qtype: QualifiedName<'a>) -> QualifiedName<'a> {
        while let Some(children) = self.child_map.get(&qtype) {
            let children = children
                .iter()
                .filter(|child| self.child_adds_property(child))
                .copied()
                .collect::<Vec<_>>();
            if children.len() > 1 {
                break;
            }
            if let Some(child) = children.first() {
                qtype = *child;
            } else {
                break;
            }
        }
        qtype
    }

    /// Find `Settings.Settings` object that represents
    /// `@Redfish.Settings` annotation of data type.
    ///
    /// # Errors
    ///
    /// Returns error if setting type is not found.
    ///
    /// # Panics
    ///
    /// Should never panic. Only if edmx `SimpleIdentifier` parser is
    /// terribly broken.
    #[allow(clippy::unwrap_in_result)]
    pub fn redfish_settings_type(&self) -> Result<(QualifiedName<'a>, &'a ComplexType), Error<'a>> {
        let ns: EdmxNamespace = "Settings".parse().expect("must be parsed");
        let id: SimpleIdentifier = "Settings".parse().expect("must be parsed");
        let schema = self
            .get(&Namespace::new(&ns))
            .ok_or(Error::SettingsTypeNotFound)?;
        let (name, _) = schema
            .types
            .get_key_value(&id)
            .ok_or(Error::SettingsTypeNotFound)?;
        let qtype = QualifiedName::new(&schema.namespace, name);
        self.find_child_complex_type(qtype)
    }

    /// Find `Settings.PreferredApplyTime` object that represents
    /// `@Redfish.SettingsApplyTime` annotation of data type.
    ///
    /// # Errors
    ///
    /// Returns error if setting type is not found.
    ///
    /// # Panics
    ///
    /// Should never panic. Only if edmx `SimpleIdentifier` parser is
    /// terribly broken.
    #[allow(clippy::unwrap_in_result)]
    pub fn redfish_settings_preferred_apply_time_type(
        &self,
    ) -> Result<(QualifiedName<'a>, &'a ComplexType), Error<'a>> {
        let ns: EdmxNamespace = "Settings".parse().expect("must be parsed");
        let id: SimpleIdentifier = "PreferredApplyTime".parse().expect("must be parsed");
        let schema = self
            .get(&Namespace::new(&ns))
            .ok_or(Error::SettingsPreferredApplyTimeTypeNotFound)?;
        let (name, _) = schema
            .types
            .get_key_value(&id)
            .ok_or(Error::SettingsPreferredApplyTimeTypeNotFound)?;
        let qtype = QualifiedName::new(&schema.namespace, name);
        self.find_child_complex_type(qtype)
    }

    #[must_use]
    fn find_entity_type_by_qname(&self, qtype: &QualifiedName<'a>) -> Option<&'a EntityType> {
        self.get(&qtype.namespace)
            .and_then(|ns| ns.entity_types.get(qtype.name))
    }

    #[must_use]
    fn find_complex_type_by_qname(&self, qtype: &QualifiedName<'a>) -> Option<&'a ComplexType> {
        self.get(&qtype.namespace)
            .and_then(|ns| ns.types.get(qtype.name))
            .and_then(|t| {
                if let Type::ComplexType(ct) = t {
                    Some(ct)
                } else {
                    None
                }
            })
    }

    fn child_adds_property(&self, qtype: &QualifiedName<'_>) -> bool {
        self.find_entity_type_by_qname(qtype)
            .map(|et| {
                !et.properties.is_empty()
                    || self.child_map.get(qtype).is_some_and(|children| {
                        children.iter().any(|child| self.child_adds_property(child))
                    })
            })
            .or_else(|| {
                self.find_complex_type_by_qname(qtype).map(|ct| {
                    !ct.properties.is_empty()
                        || self.child_map.get(qtype).is_some_and(|children| {
                            children.iter().any(|child| self.child_adds_property(child))
                        })
                })
            })
            .is_some_and(identity)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::edmx::Edmx;

    #[test]
    fn schema_index_test() {
        let schemas = [
            r#"<edmx:Edmx Version="4.0">
             <edmx:DataServices>
               <Schema Namespace="Schema.v1_0_0"/>
             </edmx:DataServices>
           </edmx:Edmx>"#,
            // Two schemas per document
            r#"<edmx:Edmx Version="4.0">
             <edmx:DataServices>
               <Schema Namespace="Schema.v1_1_0"/>
               <Schema Namespace="Schema.v1_2_0"/>
             </edmx:DataServices>
           </edmx:Edmx>"#,
        ]
        .iter()
        .map(|s| Edmx::parse(*s))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

        let index = SchemaIndex::build(&schemas);
        assert!(index
            .get(&Namespace::new(&"Schema.v1_1_0".parse().unwrap()))
            .is_some());
        assert!(index
            .get(&Namespace::new(&"Schema.v1_0_0".parse().unwrap()))
            .is_some());
        assert!(index
            .get(&Namespace::new(&"Schema.v1_2_0".parse().unwrap()))
            .is_some());
        assert!(index
            .get(&Namespace::new(&"Schema.v1_3_0".parse().unwrap()))
            .is_none());
    }
}
