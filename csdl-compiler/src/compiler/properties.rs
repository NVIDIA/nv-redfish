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

use crate::compiler::Compiled;
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
use crate::edmx::PropertyName;
use crate::edmx::attribute_values::TypeName;
use crate::edmx::property::NavigationProperty as EdmxNavigationProperty;
use crate::edmx::property::Property as EdmxProperty;
use crate::edmx::property::PropertyAttrs;
use tagged_types::TaggedType;

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
                        let (compiled, typeclass) =
                            ensure_type(v.ptype.qualified_type_name().into(), ctx, &stack)
                                .map_err(Box::new)
                                .map_err(|e| Error::Property(&sp.name, e))?;
                        p.properties.push(Property {
                            name: &v.name,
                            ptype: (typeclass, (&v.ptype).into()),
                            odata: OData::new(MustHaveId::new(false), v),
                            redfish: RedfishProperty::new(v),
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
        if ctx.config.entity_type_filter.matches(&qname) {
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
                    ptype: match &v.ptype {
                        TypeName::One(_) => PropertyType::One(ptype),
                        TypeName::CollectionOf(_) => PropertyType::CollectionOf(ptype),
                    },
                    odata: OData::new(MustHaveId::new(false), v),
                    redfish: RedfishProperty::new(v),
                }));
            Ok(compiled)
        } else {
            p.nav_properties.push(NavProperty::Reference(
                &v.name,
                IsCollection::new(matches!(v.ptype, TypeName::CollectionOf(_))),
            ));
            Ok(Compiled::default())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PropertyType<'a> {
    One(QualifiedName<'a>),
    CollectionOf(QualifiedName<'a>),
}

impl<'a> PropertyType<'a> {
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

impl<'a> From<&'a TypeName> for PropertyType<'a> {
    fn from(v: &'a TypeName) -> Self {
        match v {
            TypeName::One(v) => Self::One(v.into()),
            TypeName::CollectionOf(v) => Self::CollectionOf(v.into()),
        }
    }
}

#[derive(Debug)]
pub struct Property<'a> {
    pub name: &'a PropertyName,
    pub ptype: (TypeClass, PropertyType<'a>),
    pub odata: OData<'a>,
    pub redfish: RedfishProperty,
}

impl<'a> MapType<'a> for Property<'a> {
    fn map_type<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.ptype = (self.ptype.0, self.ptype.1.map(f));
        self
    }
}

#[derive(Debug)]
pub enum NavProperty<'a> {
    Expandable(NavPropertyExpandable<'a>),
    Reference(&'a PropertyName, IsCollection),
}

impl<'a> NavProperty<'a> {
    /// Name of the property regrardless enum variant.
    #[must_use]
    pub const fn name(&'a self) -> &'a PropertyName {
        match self {
            Self::Expandable(v) => v.name,
            Self::Reference(n, _) => n,
        }
    }
}

/// Property is collection.
pub type IsCollection = TaggedType<bool, IsCollectionTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
#[transparent(Debug)]
#[capability(inner_access)]
pub enum IsCollectionTag {}

#[derive(Debug)]
pub struct NavPropertyExpandable<'a> {
    pub name: &'a PropertyName,
    pub ptype: PropertyType<'a>,
    pub odata: OData<'a>,
    pub redfish: RedfishProperty,
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
