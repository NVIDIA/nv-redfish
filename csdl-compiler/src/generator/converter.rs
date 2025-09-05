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

//! EDMX to Redfish domain model converter
//!
//! This module handles the conversion from parsed EDMX structures to the Redfish domain model

use super::{
    Capabilities, CapabilityInfo, ComplexTypeData, Constraints, EnumData, EnumMember, ItemMetadata,
    NavigationPropertyData, Permission, PropertyData, PropertyType, RedfishResource,
    ReferencedType, ResourceItem, ResourceReference, Version, VersionedField,
};
use crate::edmx::{
    Annotation, Edmx, TypeName,
    property::PropertyAttrs,
    schema::{Schema, Type},
};
use crate::odata::annotations::{Description, LongDescription, ODataAnnotations as _};
use alloc::rc::Rc;
use std::collections::HashMap;
use tagged_types::TaggedType;

#[derive(Debug)]
pub struct RedfishTypeRegistry {
    pub versioned_types: Vec<Rc<VersionedField<ReferencedType>>>,
    versioned_lookup: HashMap<String, Rc<VersionedField<ReferencedType>>>,
}

impl RedfishTypeRegistry {
    fn new() -> Self {
        Self {
            versioned_types: Vec::new(),
            versioned_lookup: HashMap::new(),
        }
    }

    fn add_versioned_type(&mut self, versioned_type: VersionedField<ReferencedType>) {
        let rc_type = Rc::new(versioned_type);

        // Add to lookup by type name
        let type_name = match &rc_type.field {
            ReferencedType::ComplexType(complex_type) => &complex_type.metadata.name,
            ReferencedType::Enum(enum_type) => &enum_type.metadata.name,
        };
        self.versioned_lookup
            .insert(type_name.clone(), Rc::clone(&rc_type));

        self.versioned_types.push(rc_type);
    }

    fn find_type(&self, type_name: &str) -> Option<ResourceReference> {
        // Check versioned types first (direct name match)
        if let Some(found) = self.versioned_lookup.get(type_name) {
            return Some(ResourceReference::LocalType(Rc::clone(found)));
        }

        // Check for fully qualified name matches in versioned types
        for rc_type in self.versioned_lookup.values() {
            let matches = match &rc_type.field {
                ReferencedType::ComplexType(complex_type) => {
                    type_name.ends_with(&format!(".{}", complex_type.metadata.name))
                }
                ReferencedType::Enum(enum_type) => {
                    type_name.ends_with(&format!(".{}", enum_type.metadata.name))
                }
            };
            if matches {
                return Some(ResourceReference::LocalType(Rc::clone(rc_type)));
            }
        }

        None
    }
}

type ItemsFromSchema = (
    Vec<VersionedField<ResourceItem>>,
    Vec<VersionedField<ReferencedType>>,
);

impl RedfishResource {
    /// # Errors
    /// TODO: Errors from generated code, proper error and code
    pub fn from_edmx(edmx: &Edmx) -> Result<(Vec<Self>, RedfishTypeRegistry), String> {
        let mut resources = Vec::new();
        let mut type_registry = RedfishTypeRegistry::new();

        let mut base_metadata: HashMap<
            TypeName,
            (
                Description,
                Option<LongDescription>,
                Vec<String>,
                Capabilities,
            ),
        > = HashMap::new();

        for schema in &edmx.data_services.schemas {
            if Self::parse_version_from_namespace(&schema.namespace)?.is_none() {
                let resource_name = TypeName::new(
                    schema
                        .namespace
                        .split('.')
                        .next()
                        .unwrap_or(&schema.namespace)
                        .to_string(),
                );

                for schema_type in schema.types.values() {
                    if let Type::EntityType(entity_type) = schema_type {
                        if entity_type.name == resource_name {
                            let description = entity_type.odata_description_or_default();

                            let long_description =
                                entity_type.odata_long_description().map(TaggedType::cloned);

                            let mut uris = Vec::new();
                            for annotation in &entity_type.annotations {
                                if annotation.term == "Redfish.Uris" {
                                    if let Some(collection) = &annotation.collection {
                                        uris.extend(collection.strings.clone());
                                    }
                                    break;
                                }
                            }

                            let capabilities = Self::extract_capabilities(&entity_type.annotations);

                            base_metadata.insert(
                                resource_name.clone(),
                                (description, long_description, uris, capabilities),
                            );
                            break;
                        }
                    }
                }
            }
        }

        let mut resource_map: HashMap<TypeName, Self> = HashMap::new();

        for schema in &edmx.data_services.schemas {
            if let Some(version) = Self::parse_version_from_namespace(&schema.namespace)? {
                let resource_name = TypeName::new(
                    schema
                        .namespace
                        .split('.')
                        .next()
                        .unwrap_or(&schema.namespace)
                        .to_string(),
                );

                let resource = resource_map
                    .entry(resource_name.clone())
                    .or_insert_with(|| {
                        let mut new_resource = Self {
                            metadata: ItemMetadata {
                                name: resource_name.inner().clone(),
                                description: Description::new(format!("Resource {resource_name}")),
                                long_description: None,
                            },
                            uris: Vec::new(),
                            items: Vec::new(),
                            capabilities: Capabilities {
                                deletable: None,
                                insertable: None,
                                updatable: None,
                            },
                        };

                        if let Some((description, long_description, uris, capabilities)) =
                            base_metadata.get(&resource_name)
                        {
                            new_resource.metadata.description.clone_from(description);
                            new_resource
                                .metadata
                                .long_description
                                .clone_from(long_description);
                            new_resource.uris.clone_from(uris);
                            new_resource.capabilities.clone_from(capabilities);
                        }

                        new_resource
                    });

                let (resource_items, referenced_types) =
                    Self::extract_items_from_schema(schema, &version)?;
                resource.items.extend(resource_items);
                for ref_type in referenced_types {
                    type_registry.add_versioned_type(ref_type);
                }
            }
        }

        resources.extend(resource_map.into_values());

        // Resolve type references
        let resolved_resources = Self::resolve_type_references(resources, &type_registry)?;

        Ok((resolved_resources, type_registry))
    }

    fn resolve_type_references(
        resources: Vec<Self>,
        type_registry: &RedfishTypeRegistry,
    ) -> Result<Vec<Self>, String> {
        let mut resolved_resources = Vec::new();

        for resource in resources {
            let resolved_items = resource
                .items
                .into_iter()
                .map(|versioned_item| {
                    let resolved_field = Self::resolve_resource_item_references(
                        versioned_item.field,
                        type_registry,
                    )?;
                    Ok(VersionedField {
                        field: resolved_field,
                        introduced_in: versioned_item.introduced_in,
                        deprecated_in: versioned_item.deprecated_in,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;

            resolved_resources.push(Self {
                metadata: resource.metadata,
                uris: resource.uris,
                items: resolved_items,
                capabilities: resource.capabilities,
            });
        }

        Ok(resolved_resources)
    }

    fn resolve_resource_item_references(
        item: ResourceItem,
        type_registry: &RedfishTypeRegistry,
    ) -> Result<ResourceItem, String> {
        match item {
            ResourceItem::Property(mut property_data) => {
                property_data.property_type = Self::resolve_property_type_references(
                    property_data.property_type,
                    type_registry,
                )?;
                Ok(ResourceItem::Property(property_data))
            }
            ResourceItem::NavigationProperty(mut nav_data) => {
                nav_data.target_type =
                    Self::resolve_type_reference(&nav_data.target_type, type_registry)?;
                Ok(ResourceItem::NavigationProperty(nav_data))
            }
            ResourceItem::Action(action_data) => {
                // TODO: Resolve action parameter types when actions are fully implemented
                Ok(ResourceItem::Action(action_data))
            }
        }
    }

    fn resolve_property_type_references(
        property_type: PropertyType,
        type_registry: &RedfishTypeRegistry,
    ) -> Result<PropertyType, String> {
        match property_type {
            PropertyType::Collection(inner_type) => {
                let resolved_inner =
                    Self::resolve_property_type_references((*inner_type).clone(), type_registry)?;
                Ok(PropertyType::Collection(Rc::new(resolved_inner)))
            }
            PropertyType::Reference(type_ref) => {
                let resolved_ref = Self::resolve_type_reference(&type_ref, type_registry)?;
                Ok(PropertyType::Reference(resolved_ref))
            }
            other => {
                // Primitive types don't need resolution
                Ok(other)
            }
        }
    }

    fn resolve_type_reference(
        type_ref: &ResourceReference,
        type_registry: &RedfishTypeRegistry,
    ) -> Result<ResourceReference, String> {
        match type_ref {
            ResourceReference::TypeName(type_name) => {
                // Handle collection syntax
                if type_name.starts_with("Collection(") {
                    let inner_type = &type_name[11..type_name.len() - 1];
                    let resolved_inner = Self::resolve_type_reference(
                        &ResourceReference::TypeName(inner_type.to_string()),
                        type_registry,
                    )?;
                    return Ok(resolved_inner);
                }

                // Look for the type in our registry
                if let Some(found_ref) = type_registry.find_type(type_name) {
                    return Ok(found_ref);
                }

                // If not found, keep as TypeName (might be external type)
                Ok(ResourceReference::TypeName(type_name.to_string()))
            }
            // Other reference types are already resolved - Rc::clone is cheap (just reference counting)
            ResourceReference::LocalType(rc_type) => {
                Ok(ResourceReference::LocalType(Rc::clone(rc_type)))
            }
            ResourceReference::External(rc_resource) => {
                Ok(ResourceReference::External(Rc::clone(rc_resource)))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn extract_items_from_schema(
        schema: &Schema,
        version: &Version,
    ) -> Result<ItemsFromSchema, String> {
        let mut resource_items = Vec::new();
        let mut referenced_types = Vec::new();

        for schema_type in schema.types.values() {
            match schema_type {
                Type::EntityType(entity_type) => {
                    for property in &entity_type.properties {
                        match &property.attrs {
                            PropertyAttrs::StructuralProperty(structural_prop) => {
                                let item = ResourceItem::Property(PropertyData {
                                    metadata: ItemMetadata::new(
                                        property.name.clone(),
                                        structural_prop,
                                    ),
                                    property_type: Self::convert_property_type(
                                        &structural_prop.ptype,
                                    )?,
                                    nullable: structural_prop.nullable,
                                    permissions: Self::convert_permissions(
                                        &structural_prop.annotations,
                                    ),
                                    units: structural_prop
                                        .annotations
                                        .iter()
                                        .find(|a| {
                                            a.term.contains("Measures") && a.term.contains("Unit")
                                        })
                                        .and_then(|a| a.string.clone()),
                                    constraints: Self::extract_constraints(
                                        &structural_prop.annotations,
                                    ),
                                });
                                resource_items.push(VersionedField {
                                    field: item,
                                    introduced_in: version.clone(),
                                    deprecated_in: None,
                                });
                            }
                            PropertyAttrs::NavigationProperty(nav_prop) => {
                                let item =
                                    ResourceItem::NavigationProperty(NavigationPropertyData {
                                        metadata: ItemMetadata::new(
                                            property.name.clone(),
                                            nav_prop,
                                        ),
                                        target_type: ResourceReference::TypeName(
                                            nav_prop.ptype.inner().clone(),
                                        ),
                                        is_collection: nav_prop
                                            .ptype
                                            .inner()
                                            .starts_with("Collection("),
                                        nullable: nav_prop.nullable,
                                        permissions: Self::convert_permissions(
                                            &nav_prop.annotations,
                                        ),
                                        auto_expand: nav_prop
                                            .annotations
                                            .iter()
                                            .any(|a| a.term.contains("AutoExpand")),
                                        excerpt_copy: None,
                                    });
                                resource_items.push(VersionedField {
                                    field: item,
                                    introduced_in: version.clone(),
                                    deprecated_in: None,
                                });
                            }
                        }
                    }
                }
                Type::ComplexType(complex_type) => {
                    let mut properties = Vec::new();
                    let mut navigation_properties = Vec::new();

                    for property in &complex_type.properties {
                        match &property.attrs {
                            PropertyAttrs::StructuralProperty(structural_prop) => {
                                properties.push(PropertyData {
                                    metadata: ItemMetadata::new(
                                        property.name.clone(),
                                        structural_prop,
                                    ),
                                    property_type: Self::convert_property_type(
                                        &structural_prop.ptype,
                                    )?,
                                    nullable: structural_prop.nullable,
                                    permissions: Self::convert_permissions(
                                        &structural_prop.annotations,
                                    ),
                                    units: structural_prop
                                        .annotations
                                        .iter()
                                        .find(|a| {
                                            a.term.contains("Measures") && a.term.contains("Unit")
                                        })
                                        .and_then(|a| a.string.clone()),
                                    constraints: Self::extract_constraints(
                                        &structural_prop.annotations,
                                    ),
                                });
                            }
                            PropertyAttrs::NavigationProperty(nav_prop) => {
                                navigation_properties.push(NavigationPropertyData {
                                    metadata: ItemMetadata::new(property.name.clone(), nav_prop),
                                    target_type: ResourceReference::TypeName(
                                        nav_prop.ptype.inner().clone(),
                                    ),
                                    is_collection: nav_prop
                                        .ptype
                                        .inner()
                                        .starts_with("Collection("),
                                    nullable: nav_prop.nullable,
                                    permissions: Self::convert_permissions(&nav_prop.annotations),
                                    auto_expand: nav_prop
                                        .annotations
                                        .iter()
                                        .any(|a| a.term.contains("AutoExpand")),
                                    excerpt_copy: None,
                                });
                            }
                        }
                    }

                    let item = ReferencedType::ComplexType(ComplexTypeData {
                        metadata: ItemMetadata::new(
                            complex_type.name.inner().clone(),
                            complex_type,
                        ),
                        base_type: None, // TODO: Need full types support
                        properties,
                        navigation_properties,
                        additional_properties: complex_type
                            .annotations
                            .iter()
                            .find(|a| a.term == "OData.AdditionalProperties")
                            .and_then(|a| a.bool_value)
                            .unwrap_or(false),
                    });
                    referenced_types.push(VersionedField {
                        field: item,
                        introduced_in: version.clone(),
                        deprecated_in: None,
                    });
                }
                Type::EnumType(enum_type) => {
                    let item = ReferencedType::Enum(EnumData {
                        metadata: ItemMetadata::new(enum_type.name.inner().clone(), enum_type),
                        members: enum_type
                            .members
                            .iter()
                            .map(|member| EnumMember {
                                name: member.name.clone(),
                                description: member.odata_description().map(TaggedType::cloned),
                            })
                            .collect(),
                    });
                    referenced_types.push(VersionedField {
                        field: item,
                        introduced_in: version.clone(),
                        deprecated_in: None,
                    });
                }
                _ => {} // TODO: Add support for Actions in parser
            }
        }

        Ok((resource_items, referenced_types))
    }

    fn parse_version_from_namespace(namespace: &str) -> Result<Option<Version>, String> {
        if !namespace.contains('.') {
            return Ok(None);
        }

        let version_part = namespace.split('.').nth(1);

        if let Some(version_str) = version_part {
            if let Some(version_str) = version_str.strip_prefix('v') {
                let version_numbers: Vec<&str> = version_str.split('_').collect();
                if version_numbers.len() >= 3 {
                    let major = version_numbers[0]
                        .parse()
                        .map_err(|_| format!("Invalid major version: {}", version_numbers[0]))?;
                    let minor = version_numbers[1]
                        .parse()
                        .map_err(|_| format!("Invalid minor version: {}", version_numbers[1]))?;
                    let patch = version_numbers[2]
                        .parse()
                        .map_err(|_| format!("Invalid patch version: {}", version_numbers[2]))?;
                    return Ok(Some(Version {
                        major,
                        minor,
                        patch,
                    }));
                }
            }
        }

        Ok(None)
    }

    fn convert_property_type(property_type: &TypeName) -> Result<PropertyType, String> {
        match property_type.inner().as_str() {
            "Edm.String" => Ok(PropertyType::String),
            "Edm.Boolean" => Ok(PropertyType::Boolean),
            "Edm.Decimal" => Ok(PropertyType::Decimal),
            "Edm.Int32" => Ok(PropertyType::Int32),
            "Edm.Int64" => Ok(PropertyType::Int64),
            _ if property_type.inner().starts_with("Collection(") => {
                let inner_type = &property_type.inner()[11..property_type.inner().len() - 1];
                Ok(PropertyType::Collection(Rc::new(
                    Self::convert_property_type(&TypeName::new(inner_type.into()))?,
                )))
            }
            _ => {
                // TODO: Temporry, need actual types
                Ok(PropertyType::Reference(ResourceReference::TypeName(
                    property_type.to_string(),
                )))
            }
        }
    }

    fn convert_permissions(annotations: &[Annotation]) -> Permission {
        for annotation in annotations {
            if annotation.term == "OData.Permissions" {
                if let Some(enum_member) = &annotation.enum_member {
                    return match enum_member.as_str() {
                        "OData.Permission/Read" => Permission::Read,
                        "OData.Permission/Write" => Permission::Write,
                        "OData.Permission/ReadWrite" => Permission::ReadWrite,
                        _ => Permission::None,
                    };
                }
            }
        }
        Permission::None
    }

    fn extract_constraints(annotations: &[Annotation]) -> Option<Constraints> {
        let mut minimum = None;
        let mut maximum = None;
        let mut pattern = None;

        for annotation in annotations {
            match annotation.term.as_str() {
                "Validation.Minimum" => {
                    if let Some(int_val) = annotation.int_value {
                        minimum = Some(int_val);
                    }
                }
                "Validation.Maximum" => {
                    if let Some(int_val) = annotation.int_value {
                        maximum = Some(int_val);
                    }
                }
                "Validation.Pattern" => {
                    pattern.clone_from(&annotation.string);
                }
                _ => {}
            }
        }

        if minimum.is_some() || maximum.is_some() || pattern.is_some() {
            Some(Constraints {
                minimum,
                maximum,
                pattern,
            })
        } else {
            None
        }
    }

    fn extract_capabilities(annotations: &[Annotation]) -> Capabilities {
        let mut insertable = None;
        let mut updatable = None;
        let mut deletable = None;

        for annotation in annotations {
            match annotation.term.as_str() {
                "Capabilities.InsertRestrictions" => {
                    if let Some(record) = &annotation.record {
                        if record.property_value.property == "Insertable" {
                            if let Some(enabled) = record.property_value.bool_value {
                                let description = record
                                    .annotations
                                    .iter()
                                    .find(|a| a.term == "OData.Description")
                                    .and_then(|a| a.string.clone());
                                insertable = Some(CapabilityInfo {
                                    enabled,
                                    description,
                                });
                            }
                        }
                    }
                }
                "Capabilities.UpdateRestrictions" => {
                    if let Some(record) = &annotation.record {
                        if record.property_value.property == "Updatable" {
                            if let Some(enabled) = record.property_value.bool_value {
                                let description = record
                                    .annotations
                                    .iter()
                                    .find(|a| a.term == "OData.Description")
                                    .and_then(|a| a.string.clone())
                                    .or_else(|| {
                                        // Check for description at the record level
                                        record
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.Description")
                                            .and_then(|a| a.string.clone())
                                    });
                                updatable = Some(CapabilityInfo {
                                    enabled,
                                    description,
                                });
                            }
                        }
                    }
                }
                "Capabilities.DeleteRestrictions" => {
                    if let Some(record) = &annotation.record {
                        if record.property_value.property == "Deletable" {
                            if let Some(enabled) = record.property_value.bool_value {
                                let description = record
                                    .annotations
                                    .iter()
                                    .find(|a| a.term == "OData.Description")
                                    .and_then(|a| a.string.clone());
                                deletable = Some(CapabilityInfo {
                                    enabled,
                                    description,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Capabilities {
            insertable,
            updatable,
            deletable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edmx::Edmx;
    use std::fs;

    fn crate_root() -> std::path::PathBuf {
        std::path::Path::new(&env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    #[ignore]
    #[test]
    fn test_edmx_to_redfish_conversion() -> Result<(), Box<dyn std::error::Error>> {
        let data = fs::read_to_string(
            crate_root().join("test-data/redfish-schema/CoolantConnector_v1.xml"),
        )?;

        let edmx: Edmx = Edmx::parse(&data).map_err(|e| format!("EDMX parse error: {:?}", e))?;

        let (resources, type_registry) = RedfishResource::from_edmx(&edmx)?;

        assert!(
            !resources.is_empty(),
            "Should have converted at least one resource"
        );

        let coolant_connector = resources
            .iter()
            .find(|r| r.metadata.name == "CoolantConnector")
            .expect("Should have found CoolantConnector resource");

        assert_eq!(coolant_connector.metadata.name, "CoolantConnector");
        assert!(
            !coolant_connector.items.is_empty(),
            "Should have schema items"
        );

        assert!(
            coolant_connector
                .metadata
                .description
                .inner()
                .contains("liquid coolant connector")
        );
        assert!(coolant_connector.metadata.long_description.is_some());

        assert!(
            !coolant_connector.uris.is_empty(),
            "Should have URI patterns"
        );
        assert!(
            coolant_connector
                .uris
                .iter()
                .any(|uri| uri.contains("CoolantConnectors"))
        );

        let properties_count = coolant_connector
            .items
            .iter()
            .filter(|item| matches!(item.field, ResourceItem::Property(_)))
            .count();

        let nav_properties_count = coolant_connector
            .items
            .iter()
            .filter(|item| matches!(item.field, ResourceItem::NavigationProperty(_)))
            .count();

        assert!(
            properties_count > 0 || nav_properties_count > 0,
            "Should have at least some properties or navigation properties"
        );

        // Verify type registry has ComplexTypes and Enums
        let versioned_complex_types_count = type_registry
            .versioned_types
            .iter()
            .filter(|t| matches!(t.field, ReferencedType::ComplexType(_)))
            .count();

        let versioned_enums_count = type_registry
            .versioned_types
            .iter()
            .filter(|t| matches!(t.field, ReferencedType::Enum(_)))
            .count();

        let total_complex_types = versioned_complex_types_count;
        let total_enums = versioned_enums_count;

        assert!(
            total_complex_types > 0,
            "Should have ComplexTypes in type registry"
        );
        assert!(total_enums > 0, "Should have Enums in type registry");

        println!("=== Resources ===");
        println!("{coolant_connector:#?}");
        println!("\n=== Type Registry ===");
        println!(
            "Versioned ComplexTypes: {}, Versioned Enums: {}",
            versioned_complex_types_count, versioned_enums_count
        );
        println!(
            "Total ComplexTypes: {}, Total Enums: {}",
            total_complex_types, total_enums
        );

        Ok(())
    }
}
