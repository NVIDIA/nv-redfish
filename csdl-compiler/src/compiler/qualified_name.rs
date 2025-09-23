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

use crate::compiler::Namespace;
use crate::edmx::QualifiedTypeName;
use crate::edmx::attribute_values::Namespace as EdmxNamespace;
use crate::edmx::attribute_values::SimpleIdentifier;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Compiled qualified name
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct QualifiedName<'a> {
    /// Namespace where name is located.
    pub namespace: Namespace<'a>,
    /// Name.
    pub name: &'a SimpleIdentifier,
}

impl<'a> QualifiedName<'a> {
    /// Create new qualified name.
    #[must_use]
    pub const fn new(namespace: &'a EdmxNamespace, name: &'a SimpleIdentifier) -> Self {
        Self {
            namespace: Namespace::new(namespace),
            name,
        }
    }
}

impl<'a> From<&'a QualifiedTypeName> for QualifiedName<'a> {
    fn from(v: &'a QualifiedTypeName) -> Self {
        Self {
            namespace: Namespace::new(&v.inner().namespace),
            name: &v.inner().name,
        }
    }
}

impl Display for QualifiedName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}.{}", self.namespace, self.name)
    }
}
