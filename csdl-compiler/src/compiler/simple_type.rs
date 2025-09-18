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

use crate::compiler::CompiledOData;
use crate::compiler::QualifiedName;
use crate::edmx::enum_type::EnumMember;
use crate::edmx::enum_type::EnumMemberName;
use crate::edmx::enum_type::EnumUnderlyingType;

/// Compiled simple type (type definition or enumeration).
#[derive(Debug)]
pub struct SimpleType<'a> {
    /// Fully-qualified type name.
    pub name: QualifiedName<'a>,
    /// Attributes of the type.
    pub attrs: SimpleTypeAttrs<'a>,
}

/// Attributes of the simple type.
#[derive(Debug)]
pub enum SimpleTypeAttrs<'a> {
    /// Attributes of the type definition.
    TypeDefinition(CompiledTypeDefinition<'a>),
    /// Attributes of the enumeration.
    EnumType(CompiledEnumType<'a>),
}

/// Compiled type definition.
#[derive(Debug)]
pub struct CompiledTypeDefinition<'a> {
    /// Fully-qualified type name.
    pub name: QualifiedName<'a>,
    /// Underlying type name. This is always primitive type in Edm
    /// namespace.
    pub underlying_type: QualifiedName<'a>,
}

/// Compiled enum definition.
#[derive(Debug)]
pub struct CompiledEnumType<'a> {
    /// Fully-qualified type name.
    pub name: QualifiedName<'a>,
    /// Underlying type. It is always Integer of some size.
    pub underlying_type: EnumUnderlyingType,
    /// Members of the enum.
    pub members: Vec<CompiledEnumMember<'a>>,
    /// `OData` annotations associated with enum type.
    pub odata: CompiledOData<'a>,
}

/// Compiled member of the enum type.
#[derive(Debug)]
pub struct CompiledEnumMember<'a> {
    /// Name of the member.
    pub name: &'a EnumMemberName,
    /// Attached Odata annotations.
    pub odata: CompiledOData<'a>,
}

impl<'a> From<&'a EnumMember> for CompiledEnumMember<'a> {
    fn from(v: &'a EnumMember) -> Self {
        Self {
            name: &v.name,
            odata: CompiledOData::new(v),
        }
    }
}
