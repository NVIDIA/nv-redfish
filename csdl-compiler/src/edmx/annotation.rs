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

//! Deserialization and validation of Annotations

use crate::edmx::QualifiedTypeName;
use crate::edmx::attribute_values::Error as AttributeValuesError;
use crate::edmx::enum_type::EnumMemberName;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error as DeError;
use serde::de::Visitor;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::str::FromStr;

/// 14.3 Element edm:Annotation
#[derive(Debug, Deserialize)]
pub struct Annotation {
    /// 14.3.1 Attribute Term
    #[serde(rename = "@Term")]
    pub term: QualifiedTypeName,
    #[serde(rename = "@String")]
    pub string: Option<String>,
    #[serde(rename = "@Bool")]
    pub bool_value: Option<bool>,
    #[serde(rename = "@Int")]
    pub int_value: Option<i64>,
    #[serde(rename = "@EnumMember")]
    pub enum_member: Option<Box<AnnotationEnumMember>>,
    #[serde(rename = "Collection")]
    pub collection: Option<AnnotationCollection>,
    #[serde(rename = "Record")]
    pub record: Option<AnnotationRecord>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationCollection {
    #[serde(rename = "String", default)]
    pub strings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationRecord {
    #[serde(rename = "PropertyValue")]
    pub property_value: PropertyValue,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct PropertyValue {
    #[serde(rename = "@Property")]
    pub property: String,
    #[serde(rename = "@Bool")]
    pub bool_value: Option<bool>,
    #[serde(rename = "@String")]
    pub string_value: Option<String>,
    #[serde(rename = "@Int")]
    pub int_value: Option<i64>,
}

#[derive(Debug)]
pub enum Error {
    NoForwardSlash,
    NoEnumMemberName,
    BadTypeName(AttributeValuesError),
    BadMemberName(AttributeValuesError),
    InvalidEnumMember(String, Box<Error>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NoForwardSlash => "no forward slash (/) in string".fmt(f),
            Self::NoEnumMemberName => "no enum member in string".fmt(f),
            Self::BadTypeName(e) => write!(f, "bad enum type name: {e}"),
            Self::BadMemberName(e) => write!(f, "bad enum member name: {e}"),
            Self::InvalidEnumMember(s, e) => write!(f, "invalid enum memeber: {s}: {e}"),
        }
    }
}

/// 14.4.7 Expression edm:EnumMember
///
/// Note that spec gives possiblitity of more than one space-separated
/// member for `IsFlags` enum. But it is not used so we keep support
/// only one member here.
#[derive(Debug)]
pub struct AnnotationEnumMember {
    pub tname: QualifiedTypeName,
    pub mname: EnumMemberName,
}

impl FromStr for AnnotationEnumMember {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split('/');
        let tname = iter
            .next()
            .ok_or(Error::NoForwardSlash)
            .and_then(|qname_str| qname_str.parse().map_err(Error::BadTypeName))
            .map_err(|e| Error::InvalidEnumMember(s.into(), Box::new(e)))?;
        let mname = iter
            .next()
            .ok_or(Error::NoEnumMemberName)
            .and_then(|mname_str| mname_str.parse().map_err(Error::BadMemberName))
            .map_err(|e| Error::InvalidEnumMember(s.into(), Box::new(e)))?;
        Ok(Self { tname, mname })
    }
}

impl<'de> Deserialize<'de> for AnnotationEnumMember {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct EmVisitor {}
        impl Visitor<'_> for EmVisitor {
            type Value = AnnotationEnumMember;

            fn expecting(&self, formatter: &mut Formatter) -> FmtResult {
                formatter.write_str("Annotation enum member string")
            }
            fn visit_str<E: DeError>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(DeError::custom)
            }
        }

        de.deserialize_string(EmVisitor {})
    }
}
