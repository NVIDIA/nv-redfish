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

//! Compilation context (immutable).

use crate::compiler::QualifiedName;
use crate::compiler::SchemaIndex;
use crate::edmx::attribute_values::SimpleIdentifier;
use std::collections::HashSet;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::str::FromStr;

/// Compilation context
///
/// Compliation context consist of immutable data that is passed to
/// all function responsible for compilation.
///
/// Note: compilation `Stack` that represents "mutable" state of the
/// compilation.
pub struct Context<'a> {
    pub schema_index: SchemaIndex<'a>,
    pub config: Config,
}

/// Configuration of the compilation
#[derive(Default)]
pub struct Config {
    pub entity_type_filter: EntityTypeFilter,
}

#[derive(Default)]
pub struct EntityTypeFilter {
    patterns: Vec<EntityTypeFilterPattern>,
}

impl EntityTypeFilter {
    /// Create new filter for vector of patterns
    #[must_use]
    pub const fn new(patterns: Vec<EntityTypeFilterPattern>) -> Self {
        Self { patterns }
    }

    /// Check if filter matches name.
    #[must_use]
    pub fn matches(&self, typename: &QualifiedName<'_>) -> bool {
        self.patterns.is_empty() || self.patterns.iter().any(|p| p.matches(typename))
    }
}

/// Qualified name pattens
///
/// Possible patterns:
/// `ServiceRoot.*.*` - any `EntityType` in any version of service root
/// `SomeNamespace.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of namespace `SomeNamespace`
/// `*.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of any namespaces
#[derive(Clone, Debug)]
pub struct EntityTypeFilterPattern {
    ns_ids: Vec<Option<SimpleIdentifier>>,
    names: HashSet<SimpleIdentifier>,
}

impl EntityTypeFilterPattern {
    #[must_use]
    pub fn matches(&self, typename: &QualifiedName) -> bool {
        if !self.names.is_empty() && !self.names.contains(typename.name) {
            return false;
        }
        if typename.namespace.len() != self.ns_ids.len() {
            return false;
        }
        for depth in 0..typename.namespace.len() {
            if let Some(pattern_id) = &self.ns_ids[depth] {
                if let Some(ns) = typename.namespace.get_id(depth) {
                    if pattern_id != ns {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl FromStr for EntityTypeFilterPattern {
    type Err = FilterPatternError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ids = s.split('.').collect::<Vec<_>>();
        if let Some(name_pattern) = ids.pop() {
            let names = if name_pattern == "*" {
                HashSet::new()
            } else {
                name_pattern
                    .split('|')
                    .map(|id| {
                        id.parse::<SimpleIdentifier>()
                            .map_err(|_| FilterPatternError::InvalidIdentifier(id.into()))
                    })
                    .collect::<Result<HashSet<_>, _>>()?
            };
            let ns_ids = ids
                .into_iter()
                .map(|id| {
                    if id == "*" {
                        Ok(None)
                    } else {
                        id.parse()
                            .map(Some)
                            .map_err(|_| FilterPatternError::InvalidIdentifier(id.into()))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Self { ns_ids, names })
        } else {
            Err(FilterPatternError::EmptyPattern)
        }
    }
}

#[derive(Debug)]
pub enum FilterPatternError {
    EmptyPattern,
    InvalidIdentifier(String),
}

impl StdError for FilterPatternError {}

impl Display for FilterPatternError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::EmptyPattern => write!(f, "empty pattern is forbidden"),
            Self::InvalidIdentifier(v) => write!(f, "invalid pattern: {v}"),
        }
    }
}
