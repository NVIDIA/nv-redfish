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

use crate::edmx::attribute_values::Namespace as EdmxNamespace;
use crate::edmx::attribute_values::SimpleIdentifier;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::hash::Hash;
use std::hash::Hasher;

/// This namespace is wrapper around original namespace. It adds
/// additional useful possibilties like pruning tail elements of namespace.
#[derive(Clone, Copy)]
pub struct Namespace<'a> {
    edmx_ns: &'a EdmxNamespace,
    len: usize,
}

#[allow(clippy::len_without_is_empty)] // CompiledNamespace cannot be empty.
impl<'a> Namespace<'a> {
    /// Creates new compiled namespace.
    #[must_use]
    pub const fn new(edmx_ns: &'a EdmxNamespace) -> Self {
        Self {
            edmx_ns,
            len: edmx_ns.ids.len(),
        }
    }

    /// Number of ids in the namespace.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Get identifier on the specified position.
    #[must_use]
    pub fn get_id(&self, depth: usize) -> Option<&'a SimpleIdentifier> {
        if self.len > depth {
            Some(&self.edmx_ns.ids[depth])
        } else {
            None
        }
    }

    /// Get parent namespace for namespace with 2 and more elements.
    #[must_use]
    pub const fn parent(&self) -> Option<Self> {
        if self.len > 1 {
            Some(Self {
                edmx_ns: self.edmx_ns,
                len: self.len - 1,
            })
        } else {
            None
        }
    }

    /// Check if namespace is `Edm`.
    #[must_use]
    pub fn is_edm(&self) -> bool {
        self.len == 1 && self.edmx_ns.is_edm()
    }
}

impl PartialEq for Namespace<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.edmx_ns.ids[..self.len] == other.edmx_ns.ids[..self.len]
    }
}

impl Eq for Namespace<'_> {}

impl Hash for Namespace<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.edmx_ns.ids[..self.len].hash(state);
    }
}

impl Display for Namespace<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut iter = self.edmx_ns.ids[..self.len].iter();
        if let Some(v) = iter.next() {
            Display::fmt(&v, f)?;
        }
        for v in iter {
            write!(f, ".{v}")?;
        }
        Ok(())
    }
}

impl Debug for Namespace<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}
