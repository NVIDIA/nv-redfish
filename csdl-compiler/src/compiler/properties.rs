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

use crate::IsNullable;
use crate::OneOrCollection;
use crate::compiler::Compiled;
use crate::compiler::ComplexType;
use crate::compiler::Context;
use crate::compiler::EntityType;
use crate::compiler::Error;
use crate::compiler::MapType;
use crate::compiler::MustHaveId;
use crate::compiler::OData;
use crate::compiler::QualifiedName;
use crate::compiler::Stack;
use crate::compiler::TypeClass;
use crate::compiler::ensure_type;
use crate::compiler::redfish::RedfishProperty;
use crate::edmx::NavigationProperty as EdmxNavigationProperty;
use crate::edmx::PropertyName;
use crate::edmx::property::Property as EdmxProperty;
use crate::edmx::property::PropertyAttrs;
use crate::odata::annotations::Permissions;

/// Combination of all compiled properties and navigation properties.
#[derive(Default, Debug)]
pub struct Properties<'a> {
    pub properties: Vec<Property<'a>>,
    pub nav_properties: Vec<NavProperty<'a>>,
}

impl<'a> Properties<'a> {
    /// Compile properties of the object (both navigation and
    /// structural). Also it compiles all dependencies of the
    /// properties.
    ///
    /// # Errors
    ///
    /// Returens error if failed to compile and dependency.
    pub fn compile(
        props: &'a [EdmxProperty],
        ctx: &Context<'a>,
        stack: Stack<'a, '_>,
    ) -> Result<(Compiled<'a>, Self), Error<'a>> {
        props
            .iter()
            .try_fold((stack, Properties::default()), |(stack, mut p), sp| {
                let stack = match &sp.attrs {
                    PropertyAttrs::StructuralProperty(v) => {
                        let (compiled, typeinfo) =
                            ensure_type(v.ptype.qualified_type_name().into(), ctx, &stack)
                                .map_err(Box::new)
                                .map_err(|e| Error::Property(&sp.name, e))?;
                        p.properties.push(Property {
                            name: &v.name,
                            ptype: v.ptype.as_ref().map(|t| (typeinfo, t.into())),
                            odata: OData::new(MustHaveId::new(false), v),
                            redfish: RedfishProperty::new(v),
                            nullable: v.nullable.unwrap_or(IsNullable::new(false)),
                        });
                        stack.merge(compiled)
                    }
                    PropertyAttrs::NavigationProperty(v) => {
                        let compiled = Self::compile_nav_property(&mut p, v, ctx, &stack)
                            .map_err(Box::new)
                            .map_err(|e| Error::Property(&sp.name, e))?;
                        stack.merge(compiled)
                    }
                };
                Ok((stack, p))
            })
            .map(|(stack, p)| (stack.done(), p))
    }

    /// Join properties in reverse order. This function is useful when
    /// compiler have list of current object and all parents and it
    /// needs all properties in order from parent to child.
    #[must_use]
    pub fn rev_join(src: Vec<Self>) -> Self {
        let (properties, nav_properties): (Vec<_>, Vec<_>) = src
            .into_iter()
            .map(|v| (v.properties, v.nav_properties))
            .unzip();
        Self {
            properties: properties.into_iter().rev().flatten().collect(),
            nav_properties: nav_properties.into_iter().rev().flatten().collect(),
        }
    }

    /// No properties defined.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.nav_properties.is_empty()
    }

    fn compile_nav_property(
        p: &mut Self,
        v: &'a EdmxNavigationProperty,
        ctx: &Context<'a>,
        stack: &Stack<'a, '_>,
    ) -> Result<Compiled<'a>, Error<'a>> {
        let qname = v.ptype.qualified_type_name().into();
        if ctx.root_set_entities.contains(&qname) || ctx.config.entity_type_filter.matches(&qname) {
            let (ptype, compiled) = ctx
                .schema_index
                // We are searching for deepest available child in tre
                // hierarchy of types for singleton. So, we can parse most
                // recent protocol versions.
                .find_child_entity_type(qname)
                .and_then(|(qtype, et)| {
                    if stack.contains_entity(qtype) {
                        // Already compiled entity
                        Ok(Compiled::default())
                    } else {
                        EntityType::compile(qtype, et, ctx, stack)
                            .map_err(Box::new)
                            .map_err(|e| Error::EntityType(qtype, e))
                    }
                    .map(|compiled| (qtype, compiled))
                })?;
            p.nav_properties
                .push(NavProperty::Expandable(NavPropertyExpandable {
                    name: &v.name,
                    ptype: v.ptype.as_ref().map(|_| ptype),
                    odata: OData::new(MustHaveId::new(false), v),
                    redfish: RedfishProperty::new(v),
                    nullable: v.nullable.unwrap_or(IsNullable::new(false)),
                }));
            Ok(compiled)
        } else {
            p.nav_properties
                .push(NavProperty::Reference(v.ptype.as_ref().map(|_| &v.name)));
            Ok(Compiled::default())
        }
    }
}

/// Additional info about type that is used for properties.
#[derive(Clone, Copy, Debug)]
pub struct TypeInfo {
    /// Class of the type.
    pub class: TypeClass,
    /// Permissions associated with type.
    ///
    /// In Redfish Permissions on the type level is only used for two
    /// complex types (`Status` and `Condition`) in Resource namespace
    /// but this is important to support because this is in the base
    /// class of all Redfish resources.
    pub permissions: Option<Permissions>,
}

impl TypeInfo {
    /// Create simple type info.
    #[must_use]
    pub const fn simple_type() -> Self {
        Self {
            class: TypeClass::SimpleType,
            permissions: None,
        }
    }
    /// Create enum type info.
    #[must_use]
    pub const fn enum_type() -> Self {
        Self {
            class: TypeClass::EnumType,
            permissions: None,
        }
    }
    /// Create type definition info.
    #[must_use]
    pub const fn type_definition() -> Self {
        Self {
            class: TypeClass::TypeDefinition,
            permissions: None,
        }
    }
    /// Complex type info.
    #[must_use]
    pub fn complex_type(ct: &ComplexType) -> Self {
        Self {
            class: TypeClass::ComplexType,
            permissions: ct.odata.permissions.or_else(|| {
                // We can also say that complex type is read only if
                // it doesn't contain properties or all properties are
                // marked as ReadOnly.
                //
                // Note that for all properties with complex type we
                // also have type info and we also can use it as
                // read-only indicator. But it may require careful
                // handling in optimizer (it will be not enough just
                // go through all complex type and collect new type
                // info because type info depends on other type infos
                // recursively).
                if ct.properties.is_empty()
                    || ct.properties.properties.iter().all(|p| {
                        p.odata.permissions.is_some_and(|v| v == Permissions::Read)
                            || *p
                                .ptype
                                .map(|v| v.0.permissions.is_some_and(|v| v == Permissions::Read))
                                .inner()
                    })
                {
                    Some(Permissions::Read)
                } else {
                    None
                }
            }),
        }
    }
}

pub type PropertyType<'a> = OneOrCollection<(TypeInfo, QualifiedName<'a>)>;

impl<'a> PropertyType<'a> {
    /// Qualified type name of the property.
    #[must_use]
    pub const fn name(&self) -> QualifiedName<'a> {
        self.inner().1
    }
}

#[derive(Debug)]
pub struct Property<'a> {
    pub name: &'a PropertyName,
    pub ptype: PropertyType<'a>,
    pub odata: OData<'a>,
    pub redfish: RedfishProperty,
    pub nullable: IsNullable,
}

impl<'a> MapType<'a> for Property<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = self.ptype.map(|(typeclass, t)| (typeclass, f(t)));
        self
    }
}

pub type NavPropertyType<'a> = OneOrCollection<QualifiedName<'a>>;

impl<'a> NavPropertyType<'a> {
    /// Qualified type name of the property.
    #[must_use]
    pub const fn name(&self) -> QualifiedName<'a> {
        *self.inner()
    }
}

#[derive(Debug)]
pub enum NavProperty<'a> {
    Expandable(NavPropertyExpandable<'a>),
    Reference(OneOrCollection<&'a PropertyName>),
}

impl<'a> NavProperty<'a> {
    /// Name of the property regrardless enum variant.
    #[must_use]
    pub const fn name(&'a self) -> &'a PropertyName {
        match self {
            Self::Expandable(v) => v.name,
            Self::Reference(n) => n.inner(),
        }
    }
}

#[derive(Debug)]
pub struct NavPropertyExpandable<'a> {
    pub name: &'a PropertyName,
    pub ptype: NavPropertyType<'a>,
    pub odata: OData<'a>,
    pub redfish: RedfishProperty,
    pub nullable: IsNullable,
}

impl<'a> MapType<'a> for NavProperty<'a> {
    fn map_type<F>(self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        match self {
            Self::Expandable(mut exp) => {
                exp.ptype = exp.ptype.map(f);
                Self::Expandable(exp)
            }
            Self::Reference { .. } => self,
        }
    }
}
