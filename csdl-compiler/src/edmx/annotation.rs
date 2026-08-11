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

use crate::edmx::attribute_values::Error as AttributeValuesError;
use crate::edmx::EnumMemberName;
use crate::edmx::QualifiedTypeName;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde::Deserializer;
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
    /// 14.4.7 `EnumMember` expression in attribute or child-element form.
    #[serde(rename = "@EnumMember", alias = "EnumMember")]
    enum_member: Option<EnumMemberExpression>,
    #[serde(rename = "Collection")]
    pub collection: Option<AnnotationCollection>,
    #[serde(rename = "Record")]
    pub record: Option<AnnotationRecord>,
}

impl Annotation {
    /// Members from the `EnumMember` expression.
    pub fn enum_members(&self) -> impl Iterator<Item = &AnnotationEnumMember> + '_ {
        self.enum_member.iter().flat_map(|expression| &expression.0)
    }
}

#[derive(Debug, Deserialize)]
pub struct AnnotationCollection {
    #[serde(rename = "String", default)]
    pub strings: Vec<String>,
    #[serde(rename = "Record", default)]
    pub record: Vec<AnnotationRecord>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationRecord {
    #[serde(rename = "PropertyValue")]
    pub property_value: Vec<PropertyValue>,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

impl AnnotationRecord {
    #[must_use]
    pub fn property_value(&self, name: &str) -> Option<&PropertyValue> {
        self.property_value
            .iter()
            .find(|v| v.property.as_str() == name)
    }
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
    /// 14.4.7 `EnumMember` expression in attribute or child-element form.
    #[serde(rename = "@EnumMember", alias = "EnumMember")]
    enum_member: Option<EnumMemberExpression>,
}

impl PropertyValue {
    /// Members from `EnumMember` expressions in either supported XML form.
    pub fn enum_members(&self) -> impl Iterator<Item = &AnnotationEnumMember> + '_ {
        self.enum_member.iter().flat_map(|expression| &expression.0)
    }
}

#[derive(Debug)]
pub enum Error {
    NoForwardSlash,
    NoEnumMemberName,
    TrailingSegments,
    BadTypeName(AttributeValuesError),
    BadMemberName(AttributeValuesError),
    InvalidEnumMember(String, Box<Self>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NoForwardSlash => "no forward slash (/) in string".fmt(f),
            Self::NoEnumMemberName => "no enum member in string".fmt(f),
            Self::TrailingSegments => "extra segments after enum member name".fmt(f),
            Self::BadTypeName(e) => write!(f, "bad enum type name: {e}"),
            Self::BadMemberName(e) => write!(f, "bad enum member name: {e}"),
            Self::InvalidEnumMember(s, e) => write!(f, "invalid enum memeber: {s}: {e}"),
        }
    }
}

/// One member of a 14.4.7 `edm:EnumMember` expression.
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
        if iter.next().is_some() {
            return Err(Error::InvalidEnumMember(
                s.into(),
                Box::new(Error::TrailingSegments),
            ));
        }
        Ok(Self { tname, mname })
    }
}

/// A 14.4.7 `edm:EnumMember` expression.
#[derive(Debug)]
struct EnumMemberExpression(Vec<AnnotationEnumMember>);

impl<'de> Deserialize<'de> for EnumMemberExpression {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let value = String::deserialize(de)?;
        let members = value
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(DeError::custom)?;
        if members.is_empty() {
            Err(DeError::custom("empty enum member expression"))
        } else {
            Ok(Self(members))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Annotation;
    use quick_xml::de::from_str;

    #[test]
    fn enum_member_expressions_support_elements_and_flags() {
        let annotation: Annotation = from_str(
            r#"<Annotation Term="Example.Term">
                 <EnumMember>Example.Flags/One Example.Flags/Two</EnumMember>
               </Annotation>"#,
        )
        .expect("valid annotation");
        let members = annotation
            .enum_members()
            .map(|member| member.mname.inner().inner().as_str())
            .collect::<Vec<_>>();

        assert_eq!(members, ["One", "Two"]);

        assert!(from_str::<Annotation>(
            r#"<Annotation Term="Example.Term" EnumMember="Example.Flags/One/Trailing"/>"#
        )
        .is_err());
    }
}
