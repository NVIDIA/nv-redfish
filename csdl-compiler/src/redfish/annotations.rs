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

use crate::edmx::Action;
use crate::edmx::Annotation;
use crate::edmx::AnnotationEnumMember;
use crate::edmx::ComplexType;
use crate::edmx::EntityType;
use crate::edmx::EnumMember;
use crate::edmx::EnumType;
use crate::edmx::NavigationProperty;
use crate::edmx::Parameter;
use crate::edmx::QualifiedTypeName;
use crate::edmx::StructuralProperty;
use crate::redfish::Deprecation;
use crate::redfish::DynamicProperties;
use crate::redfish::Excerpt;
use crate::redfish::ExcerptCopy;
use crate::redfish::ExcerptKey;
use crate::IsExcerptCopyOnly;
use crate::IsRequired;
use crate::IsRequiredOnCreate;
use std::convert::identity;

pub trait RedfishAnnotation {
    fn is_redfish_annotation(&self, name: &str) -> bool;
}

impl RedfishAnnotation for Annotation {
    fn is_redfish_annotation(&self, name: &str) -> bool {
        is_redfish_qualified_name(&self.term, name)
    }
}

fn is_redfish_qualified_name(qname: &QualifiedTypeName, name: &str) -> bool {
    qname.inner().namespace.ids.len() == 1
        && qname.inner().namespace.ids[0].inner() == "Redfish"
        && qname.inner().name.inner() == name
}

fn is_deprecated_revision_kind(em: &AnnotationEnumMember) -> bool {
    is_redfish_qualified_name(&em.tname, "RevisionKind") && em.mname.inner().inner() == "Deprecated"
}

fn revisions_deprecation(annotations: &[Annotation]) -> Option<Deprecation> {
    annotations
        .iter()
        .find(|a| a.is_redfish_annotation("Revisions"))
        .and_then(|a| a.collection.as_ref())
        .and_then(|collection| {
            collection
                .record
                .iter()
                .find(|record| {
                    record.property_value("Kind").is_some_and(|kind| {
                        kind.enum_members()
                            .any(|em| is_deprecated_revision_kind(&em))
                    })
                })
                .map(|record| Deprecation {
                    version: record
                        .property_value("Version")
                        .and_then(|v| v.string_value.clone()),
                    description: record
                        .property_value("Description")
                        .and_then(|v| v.string_value.clone()),
                })
        })
}

fn legacy_deprecation(annotations: &[Annotation]) -> Option<Deprecation> {
    annotations
        .iter()
        .find(|a| a.is_redfish_annotation("Deprecated"))
        .map(|a| Deprecation {
            version: None,
            description: a.string.clone(),
        })
}

pub trait RedfishAnnotations {
    fn annotations(&self) -> &Vec<Annotation>;

    fn is_required(&self) -> IsRequired {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("Required"))
            .map_or_else(|| IsRequired::new(false), |_| IsRequired::new(true))
    }

    fn is_required_on_create(&self) -> IsRequiredOnCreate {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("RequiredOnCreate"))
            .map_or_else(
                || IsRequiredOnCreate::new(false),
                |_| IsRequiredOnCreate::new(true),
            )
    }

    fn is_excerpt_only(&self) -> IsExcerptCopyOnly {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("ExcerptCopyOnly"))
            .map_or_else(
                || IsExcerptCopyOnly::new(false),
                |v| IsExcerptCopyOnly::new(v.bool_value.is_none_or(identity)),
            )
    }

    /// Returns excerpt keyse of the property. If None then it is not
    /// except property.
    fn excerpt(&self) -> Option<Excerpt> {
        self.annotations()
            .iter()
            .find(|a| {
                a.is_redfish_annotation("Excerpt") || a.is_redfish_annotation("ExcerptCopyOnly")
            })
            .and_then(|v| {
                v.string.as_ref().map_or_else(
                    || Some(Excerpt::All),
                    |s| {
                        Some(Excerpt::Keys(
                            s.split(',').map(Into::into).map(ExcerptKey::new).collect(),
                        ))
                    },
                )
            })
    }

    /// Returns if property is marked as excerpt copy.
    fn excerpt_copy(&self) -> Option<ExcerptCopy> {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("ExcerptCopy"))
            .and_then(|v| {
                v.string.as_ref().map_or_else(
                    || Some(ExcerptCopy::AllKeys),
                    |s| Some(ExcerptCopy::Key(ExcerptKey::new(s.into()))),
                )
            })
    }

    /// Returns the deprecation of the property, declared either as a
    /// `Redfish.Revisions` record with `Kind` of
    /// `Redfish.RevisionKind/Deprecated` (records of other kinds,
    /// e.g. `Added`, are ignored) or as the legacy
    /// `Redfish.Deprecated` string term used by older schemas.
    fn deprecation(&self) -> Option<Deprecation> {
        revisions_deprecation(self.annotations()).or_else(|| legacy_deprecation(self.annotations()))
    }

    /// Returns if type can contain dynamic properties.
    fn dynamic_properties(&self) -> Option<DynamicProperties<'_>> {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("DynamicPropertyPatterns"))
            .and_then(|v| v.collection.as_ref())
            .and_then(|collection| {
                collection.record.iter().find_map(|record| {
                    record
                        .property_value("Type")
                        .and_then(|t| t.string_value.as_ref())
                        .and_then(|t| {
                            record
                                .property_value("Pattern")
                                .and_then(|p| p.string_value.as_ref())
                                .map(|p| DynamicProperties {
                                    pattern: p,
                                    ptype: t,
                                })
                        })
                })
            })
    }
}

impl RedfishAnnotations for StructuralProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for NavigationProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for Parameter {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for ComplexType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for EnumMember {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for EnumType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for EntityType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishAnnotations for Action {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

#[cfg(test)]
mod tests {
    use super::RedfishAnnotations as _;
    use crate::compiler::redfish::RedfishProperty;
    use crate::edmx::property::PropertyAttrs;
    use crate::edmx::Edmx;
    use crate::edmx::StructuralProperty;

    fn with_single_property<T>(
        property_xml: &str,
        check: impl FnOnce(&StructuralProperty) -> T,
    ) -> T {
        let data = format!(
            r#"
           <edmx:Edmx Version="4.0">
             <edmx:DataServices>
               <Schema Namespace="Event">
                 <EntityType Name="EventRecord">
                   {property_xml}
                 </EntityType>
               </Schema>
             </edmx:DataServices>
           </edmx:Edmx>"#
        );
        let edmx = Edmx::parse(&data).expect("valid edmx");
        let entity = edmx.data_services.schemas[0]
            .entity_types
            .values()
            .next()
            .expect("entity type");
        let property = entity
            .properties
            .first()
            .and_then(|p| match &p.attrs {
                PropertyAttrs::StructuralProperty(p) => Some(p),
                PropertyAttrs::NavigationProperty(_) => None,
            })
            .expect("structural property");
        check(property)
    }

    #[test]
    fn deprecation_parsed_from_deprecated_revision() {
        // The exact annotation shape of `Event.EventRecord/EventType`.
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String" Nullable="false">
              <Annotation Term="Redfish.Required"/>
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Redfish.RevisionKind/Deprecated"/>
                    <PropertyValue Property="Version" String="v1_3_0"/>
                    <PropertyValue Property="Description" String="This property has been deprecated."/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| {
                let deprecation = p.deprecation().expect("deprecation");
                assert_eq!(deprecation.version.as_deref(), Some("v1_3_0"));
                assert_eq!(
                    deprecation.description.as_deref(),
                    Some("This property has been deprecated.")
                );
                assert!(p.is_required().into_inner());
                // Deprecation cancels `Redfish.Required` in the
                // compiled property.
                assert!(!RedfishProperty::new(p).is_required.into_inner());
            },
        );
    }

    #[test]
    fn deprecation_found_among_other_revision_kinds() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Redfish.RevisionKind/Added"/>
                    <PropertyValue Property="Version" String="v1_1_0"/>
                  </Record>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Redfish.RevisionKind/Deprecated"/>
                    <PropertyValue Property="Version" String="v1_3_0"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| {
                let deprecation = p.deprecation().expect("deprecation");
                assert_eq!(deprecation.version.as_deref(), Some("v1_3_0"));
                assert_eq!(deprecation.description, None);
            },
        );
    }

    #[test]
    fn added_revision_is_not_deprecation() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Required"/>
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Redfish.RevisionKind/Added"/>
                    <PropertyValue Property="Version" String="v1_1_0"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| {
                assert!(p.deprecation().is_none());
                assert!(RedfishProperty::new(p).is_required.into_inner());
            },
        );
    }

    // Space-separated `IsFlags` values (OData CSDL 14.4.7) are parsed
    // member by member: they must not fail the parse, and each member
    // is checked individually.
    #[test]
    fn flags_enum_members_are_parsed_individually() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Ns.Flags/A Ns.Flags/B"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| assert!(p.deprecation().is_none()),
        );
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Ns.Flags/A Redfish.RevisionKind/Deprecated"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| assert!(p.deprecation().is_some()),
        );
    }

    // A malformed value with extra segments must not read as a valid
    // Deprecated kind.
    #[test]
    fn enum_member_with_trailing_segments_is_ignored() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind" EnumMember="Redfish.RevisionKind/Deprecated/Garbage"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| assert!(p.deprecation().is_none()),
        );
    }

    // OData CSDL 14.4.7 also allows the enum member as a child element
    // of the property value instead of an attribute.
    #[test]
    fn deprecation_parsed_from_element_form_enum_member() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind">
                      <EnumMember>Redfish.RevisionKind/Deprecated</EnumMember>
                    </PropertyValue>
                    <PropertyValue Property="Version" String="v1_3_0"/>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| {
                let deprecation = p.deprecation().expect("deprecation");
                assert_eq!(deprecation.version.as_deref(), Some("v1_3_0"));
            },
        );
    }

    // Repeated element-form enum members (the `IsFlags` shape) must
    // not fail the parse; the first supported value is used.
    #[test]
    fn repeated_element_form_enum_members_parse() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Revisions">
                <Collection>
                  <Record>
                    <PropertyValue Property="Kind">
                      <EnumMember>Ns.Flags/A Ns.Flags/B</EnumMember>
                      <EnumMember>Redfish.RevisionKind/Deprecated</EnumMember>
                    </PropertyValue>
                  </Record>
                </Collection>
              </Annotation>
            </Property>"#,
            |p| assert!(p.deprecation().is_some()),
        );
    }

    // Older schemas mark deprecation with the legacy string term
    // instead of a `Redfish.Revisions` record; it must cancel
    // `Redfish.Required` the same way.
    #[test]
    fn legacy_deprecated_term_is_deprecation() {
        with_single_property(
            r#"
            <Property Name="Encrypted" Type="Edm.String">
              <Annotation Term="Redfish.Required"/>
              <Annotation Term="Redfish.Deprecated" String="Deprecated in favor of something else."/>
            </Property>"#,
            |p| {
                let deprecation = p.deprecation().expect("deprecation");
                assert_eq!(deprecation.version, None);
                assert_eq!(
                    deprecation.description.as_deref(),
                    Some("Deprecated in favor of something else.")
                );
                assert!(!RedfishProperty::new(p).is_required.into_inner());
            },
        );
    }

    #[test]
    fn no_revisions_is_not_deprecation() {
        with_single_property(
            r#"
            <Property Name="EventType" Type="Edm.String">
              <Annotation Term="Redfish.Required"/>
            </Property>"#,
            |p| assert!(p.deprecation().is_none()),
        );
    }
}
