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

//! Types defined in 17 Attribute Values

use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error as DeError;
use serde::de::Visitor;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::str::FromStr;

#[derive(Debug)]
pub enum Error {
    InvalidSimpleIdentifier(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::InvalidSimpleIdentifier(id) => write!(f, "invalid simple identifier {id}"),
        }
    }
}

/// 17.1 Namespace
#[derive(Clone, Debug)]
pub struct Namespace {
    pub ids: Vec<SimpleIdentifier>,
}

impl FromStr for Namespace {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            ids: s
                .split('.')
                .map(SimpleIdentifier::from_str)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl Display for Namespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut iter = self.ids.iter();
        if let Some(v) = iter.next() {
            v.fmt(f)?;
        }
        for v in iter {
            ".".fmt(f)?;
            v.fmt(f)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Namespace {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct NsVisitor {}
        impl Visitor<'_> for NsVisitor {
            type Value = Namespace;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> FmtResult {
                formatter.write_str("Namespace string")
            }
            fn visit_str<E: DeError>(self, value: &str) -> Result<Namespace, E> {
                value.parse().map_err(DeError::custom)
            }
        }

        de.deserialize_string(NsVisitor {})
    }
}

#[derive(Clone, Debug)]
pub struct SimpleIdentifier(String);

impl SimpleIdentifier {
    #[must_use]
    pub const fn inner(&self) -> &String {
        &self.0
    }
}

impl Display for SimpleIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

impl FromStr for SimpleIdentifier {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();

        // Normative: starts with a letter or underscore, followed by
        // at most 127 letters, underscores or digits.
        //
        // Implementation: we don't check max length.
        chars
            .next()
            .and_then(|first| {
                if first.is_alphabetic() || first == '_' {
                    Some(())
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::InvalidSimpleIdentifier(s.into()))?;

        if chars.any(|c| !c.is_alphanumeric() && c != '_') {
            Err(Error::InvalidSimpleIdentifier(s.into()))
        } else {
            Ok(Self(s.into()))
        }
    }
}

impl<'de> Deserialize<'de> for SimpleIdentifier {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct SiVisitor {}
        impl Visitor<'_> for SiVisitor {
            type Value = SimpleIdentifier;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> FmtResult {
                formatter.write_str("SimpleIdentifier string")
            }
            fn visit_str<E: DeError>(self, value: &str) -> Result<SimpleIdentifier, E> {
                value.parse().map_err(DeError::custom)
            }
        }

        de.deserialize_string(SiVisitor {})
    }
}
