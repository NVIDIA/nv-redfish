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

use crate::compiler::ensure_type;
use crate::compiler::is_simple_type;
use crate::compiler::Compiled;
use crate::compiler::Context;
use crate::compiler::Error;
use crate::compiler::MapBase;
use crate::compiler::MustHaveId;
use crate::compiler::NavProperty;
use crate::compiler::OData;
use crate::compiler::Properties;
use crate::compiler::PropertiesManipulation;
use crate::compiler::Property;
use crate::compiler::QualifiedName;
use crate::compiler::Redfish;
use crate::compiler::Stack;
use crate::compiler::TypeInfo;
use crate::edmx::ComplexType as EdmxComplexType;
use crate::odata::annotations::Permissions;
use crate::IsAbstract;

/// Compiled complex type.
#[derive(Debug)]
pub struct ComplexType<'a> {
    /// Fully qualified type name.
    pub name: QualifiedName<'a>,
    /// Optional base complex type.
    pub base: Option<QualifiedName<'a>>,
    /// Structural and navigation properties.
    pub properties: Properties<'a>,
    /// Attached `OData` annotations.
    pub odata: OData<'a>,
    /// Attached Redfish annotations.
    pub redfish: Redfish<'a>,
    /// Whether the type is abstract.
    pub is_abstract: IsAbstract,
}

impl<'a> PropertiesManipulation<'a> for ComplexType<'a> {
    fn map_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(Property<'a>) -> Property<'a>,
    {
        self.properties.properties = self.properties.properties.into_iter().map(f).collect();
        self
    }

    fn map_nav_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(NavProperty<'a>) -> NavProperty<'a>,
    {
        self.properties.nav_properties =
            self.properties.nav_properties.into_iter().map(f).collect();
        self
    }
}

impl<'a> MapBase<'a> for ComplexType<'a> {
    fn map_base<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.base = self.base.map(f);
        self
    }
}

impl ComplexType<'_> {
    /// Whether this type own definition warrants an Update struct.
    #[must_use]
    pub fn generates_update(&self) -> bool {
        TypeInfo::complex_type(self)
            .permissions
            .is_none_or(|v| v != Permissions::Read)
            || self.is_abstract.into_inner()
    }
}

pub(crate) fn compile<'a>(
    qtype: QualifiedName<'a>,
    ct: &'a EdmxComplexType,
    ctx: &Context<'a>,
    stack: &Stack<'a, '_>,
) -> Result<(Compiled<'a>, TypeInfo), Error<'a>> {
    // The frame marks this type as in progress before anything recurses,
    // so a self-referential property terminates instead of compiling its
    // own type forever.
    let stack = stack.new_frame().with_complex_type(qtype);
    // Ensure that the base complex type is compiled, if present.
    // `ensure_type` short-circuits `Edm.*`, so an invalid simple-type
    // base is rejected here to stay a schema-time error.
    let (base, stack) = if let Some(base_type) = &ct.base_type {
        let base: QualifiedName = base_type.into();
        if is_simple_type(base) {
            return Err(Error::TypeNotFound(base));
        }
        let (compiled, _) = ensure_type(base, ctx, &stack)?;
        (Some(base), stack.merge(compiled))
    } else {
        (None, stack)
    };

    let (compiled, properties) =
        Properties::compile(qtype, &ct.properties, ctx, stack.new_frame())?;

    let complex_type = ComplexType {
        name: qtype,
        base,
        properties,
        odata: OData::new(MustHaveId::new(false), ct),
        redfish: Redfish::new(ct),
        is_abstract: ct.is_abstract,
    };
    let typeinfo = TypeInfo::complex_type(&complex_type);
    Ok((
        stack
            .merge(compiled)
            .merge(Compiled::new_complex_type(complex_type))
            .done(),
        typeinfo,
    ))
}
