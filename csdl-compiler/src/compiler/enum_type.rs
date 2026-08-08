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

use crate::compiler::odata::MustHaveId;
use crate::compiler::Compiled;
use crate::compiler::OData;
use crate::compiler::QualifiedName;
use crate::compiler::TypeInfo;
use crate::edmx::EnumMember as EdmxEnumMember;
use crate::edmx::EnumMemberName;
use crate::edmx::EnumType as EdmxEnumType;
use crate::edmx::EnumUnderlyingType;
use crate::redfish::annotations::RedfishAnnotations as _;
use crate::redfish::Deprecation;

/// Compiled enumeration type.
#[derive(Debug)]
pub struct EnumType<'a> {
    /// Fully-qualified type name.
    pub name: QualifiedName<'a>,
    /// Underlying integral type.
    pub underlying_type: EnumUnderlyingType,
    /// Members of the enum.
    pub members: Vec<EnumMember<'a>>,
    /// `OData` annotations associated with the enum type.
    pub odata: OData<'a>,
    /// Deprecation revision, if the schema marks the type deprecated.
    pub deprecation: Option<Box<Deprecation>>,
}
/// Compiled member of an enum type.
#[derive(Debug)]
pub struct EnumMember<'a> {
    /// Name of the member.
    pub name: &'a EnumMemberName,
    /// Attached `OData` annotations.
    pub odata: OData<'a>,
    /// Deprecation revision, if the schema marks the member
    /// deprecated.
    pub deprecation: Option<Box<Deprecation>>,
}

impl<'a> From<&'a EdmxEnumMember> for EnumMember<'a> {
    fn from(v: &'a EdmxEnumMember) -> Self {
        Self {
            name: &v.name,
            odata: OData::new(MustHaveId::new(false), v),
            deprecation: v.deprecation().map(Box::new),
        }
    }
}

pub(crate) fn compile<'a>(
    qtype: QualifiedName<'a>,
    et: &'a EdmxEnumType,
) -> (Compiled<'a>, TypeInfo) {
    let underlying_type = et.underlying_type.unwrap_or_default();
    (
        Compiled::new_enum_type(EnumType {
            name: qtype,
            underlying_type,
            members: et.members.iter().map(Into::into).collect(),
            odata: OData::new(MustHaveId::new(false), et),
            deprecation: et.deprecation().map(Box::new),
        }),
        TypeInfo::enum_type(),
    )
}
