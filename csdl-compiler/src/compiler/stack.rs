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

use crate::compiler::Compiled;
use crate::compiler::QualifiedName;
use crate::compiler::TypeInfo;

/// Compilation stack. Creates every time when we go inside recursion
/// to compile inner type.
///
/// Note: you should never return Stack frames from the function that
/// do part of job. This is anti-pattern. Function that compiles
/// something should create stack frame (if needed) and return
/// `Compiled` data using done method.
#[derive(Default)]
pub struct Stack<'a, 'stack> {
    parent: Option<&'stack Stack<'a, 'stack>>,
    // If entity type is currently being compiled we use this
    // field to prevent infinite recursion.
    entity_type: Option<QualifiedName<'a>>,
    current: Compiled<'a>,
}

impl<'a, 'stack> Stack<'a, 'stack> {
    /// Create new stack frame and sets it's parent to the `self`.
    /// Stack frame is concept that provides possibility to have own
    /// mutable `current` compiled data and still access upper frames
    /// to lookup for already compiled data structures.
    #[must_use]
    pub fn new_frame(&'stack self) -> Self {
        Self {
            parent: Some(self),
            entity_type: None,
            current: Compiled::default(),
        }
    }

    /// Entity types can refer to each other via navigation
    /// properties. These references can produce cycles. Stack helps
    /// avoiding infinite loops by possibility remember which entity
    /// type is being compiled. This function enables this possiblity.
    #[must_use]
    pub const fn with_enitity_type(mut self, name: QualifiedName<'a>) -> Self {
        self.entity_type = Some(name);
        self
    }

    /// Check that entity this has been compiled or is being compiled.
    #[must_use]
    pub fn contains_entity(&self, qtype: QualifiedName<'a>) -> bool {
        self.current.entity_types.contains_key(&qtype)
            || self.entity_type.is_some_and(|v| v == qtype)
            || self.parent.is_some_and(|p| p.contains_entity(qtype))
    }

    /// Check that complex type has been compiled.
    #[must_use]
    pub fn complex_type_info(&self, qtype: QualifiedName<'a>) -> Option<TypeInfo> {
        self.current
            .complex_types
            .get(&qtype)
            .map(|v| TypeInfo::complex_type(v))
            .or_else(|| self.parent.and_then(|p| p.complex_type_info(qtype)))
    }

    /// Check that type definition has been compiled.
    #[must_use]
    pub fn contains_type_definition(&self, qtype: QualifiedName<'a>) -> bool {
        self.current.type_definitions.contains_key(&qtype)
            || self
                .parent
                .is_some_and(|p| p.contains_type_definition(qtype))
    }

    /// Check that enum type has been compiled.
    #[must_use]
    pub fn contains_enum_type(&self, qtype: QualifiedName<'a>) -> bool {
        self.current.enum_types.contains_key(&qtype)
            || self.parent.is_some_and(|p| p.contains_enum_type(qtype))
    }

    /// Merge compiled data structure to the current stack frame.
    #[must_use]
    pub fn merge(self, c: Compiled<'a>) -> Self {
        Self {
            parent: self.parent,
            entity_type: self.entity_type,
            current: self.current.merge(c),
        }
    }

    /// Complete stack frame and return collected compiled data structure.
    #[must_use]
    pub fn done(self) -> Compiled<'a> {
        self.current
    }
}
