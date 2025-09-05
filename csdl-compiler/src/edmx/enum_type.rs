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

use crate::ValidateError;
use crate::edmx::Annotation;
use crate::edmx::TypeName;
use serde::Deserialize;

pub type EnumMemberName = String;

/// 10.1 Element edm:EnumType
#[derive(Debug, Deserialize)]
pub struct DeEnumType {
    /// 10.1.1 Attribute `Name`
    #[serde(rename = "@Name")]
    pub name: TypeName,
    /// 10.1.2 Attribute `UnderlyingType`
    #[serde(rename = "@UnderlyingType")]
    pub underlying_type: Option<TypeName>,
    /// 10.1.3 Attribute `IsFlags`
    #[serde(rename = "@IsFlags")]
    pub is_flags: Option<bool>,
    /// Child elements of `EnumType`.
    #[serde(rename = "$value", default)]
    pub items: Vec<DeEnumTypeItem>,
}

#[derive(Debug, Deserialize)]
pub enum DeEnumTypeItem {
    /// 10.2 Element edm:Member
    Member(EnumMember),
    /// Annotations can be in any type.
    Annotation(Annotation),
}

/// 10.2 Element edm:Member
#[derive(Debug, Deserialize)]
pub struct EnumMember {
    /// 10.2.1 Attribute Name
    #[serde(rename = "@Name")]
    pub name: EnumMemberName,
    /// 10.2.2 Attribute Value
    #[serde(rename = "@Value")]
    pub value: Option<String>,
    /// Annotations can be in any type.
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

/// Validated edm:EnumType.
#[derive(Debug)]
pub struct EnumType {
    pub name: TypeName,
    pub underlying_type: Option<TypeName>,
    pub is_flags: Option<bool>,
    pub members: Vec<EnumMember>,
    pub annotations: Vec<Annotation>,
}

impl DeEnumType {
    /// # Errors
    ///
    /// Actually, doesn't return any errors. Keeping constent calls.
    pub fn validate(self) -> Result<EnumType, ValidateError> {
        let (members, annotations) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut ms, mut anns), v| {
                    match v {
                        DeEnumTypeItem::Member(v) => ms.push(v),
                        DeEnumTypeItem::Annotation(v) => anns.push(v),
                    }
                    (ms, anns)
                });
        Ok(EnumType {
            name: self.name.clone(),
            underlying_type: self.underlying_type,
            is_flags: self.is_flags,
            members,
            annotations,
        })
    }
}
