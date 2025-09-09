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
use crate::edmx::LocalTypeName;
use crate::edmx::QualifiedTypeName;
use crate::edmx::SimpleIdentifier;
use crate::edmx::annotation::Annotation;
use crate::edmx::attribute_values::Error as QualifiedNameError;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error as DeError;
use serde::de::Visitor;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::str::FromStr;
use tagged_types::TaggedType;

pub type EnumMemberName = TaggedType<SimpleIdentifier, EnumMemberNameTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Eq, PartialEq)]
#[transparent(Deserialize, FromStr, Debug, Display)]
#[capability(inner_access)]
pub enum EnumMemberNameTag {}

/// 10.1 Element edm:EnumType
#[derive(Debug, Deserialize)]
pub struct DeEnumType {
    /// 10.1.1 Attribute `Name`
    #[serde(rename = "@Name")]
    pub name: LocalTypeName,
    /// 10.1.2 Attribute `UnderlyingType`
    #[serde(rename = "@UnderlyingType")]
    pub underlying_type: Option<EnumUnderlyingType>,
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
    pub name: LocalTypeName,
    pub underlying_type: Option<EnumUnderlyingType>,
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

#[derive(Debug)]
pub enum Error {
    Syntax(QualifiedNameError),
    BadUnderlyingType(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Syntax(err) => write!(f, "wrong enum underlying type syntax: {err}"),
            Self::BadUnderlyingType(id) => write!(f, "bad enum underlying type {id}"),
        }
    }
}

/// 10.1.2 Attribute `UnderlyingType`
#[derive(Debug, Default)]
pub enum EnumUnderlyingType {
    Byte,
    SByte,
    Int16,
    #[default]
    Int32,
    Int64,
}

impl FromStr for EnumUnderlyingType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let qname: QualifiedTypeName = s.parse().map_err(Error::Syntax)?;
        if qname.inner().namespace.is_edm() {
            match qname.inner().name.inner().as_str() {
                "Byte" => Ok(Self::Byte),
                "SByte" => Ok(Self::SByte),
                "Int16" => Ok(Self::Int16),
                "Int32" => Ok(Self::Int32),
                "Int64" => Ok(Self::Int64),
                _ => Err(Error::BadUnderlyingType(s.into())),
            }
        } else {
            Err(Error::BadUnderlyingType(s.into()))
        }
    }
}

impl<'de> Deserialize<'de> for EnumUnderlyingType {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct UtVisitor {}
        impl Visitor<'_> for UtVisitor {
            type Value = EnumUnderlyingType;

            fn expecting(&self, formatter: &mut Formatter) -> FmtResult {
                formatter.write_str("Enum UnderlyingType string")
            }
            fn visit_str<E: DeError>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(DeError::custom)
            }
        }

        de.deserialize_string(UtVisitor {})
    }
}
