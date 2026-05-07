// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Opaque dispatcher identifier types.
//!
//! Identifiers are intentionally opaque. They are allocated by the runtime
//! ([`TargetId`], [`GeneratorId`]) or constructed from an application-supplied
//! name ([`ClassId`]). The internal representation is private; only the
//! intentional accessors documented on each type are public.
//!
//! Identifiers do not expose application semantics. The runtime treats targets
//! as opaque scheduling units.

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use std::sync::Arc;

/// Opaque identifier of a runtime target.
///
/// The scheduler treats targets as opaque scheduling units. The runtime
/// allocates [`TargetId`] values via [`crate::Runtime::add_target`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetId(u64);

impl TargetId {
    pub(crate) const fn from_seq(seq: u64) -> Self {
        Self(seq)
    }
}

impl Debug for TargetId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "TargetId({})", self.0)
    }
}

impl Display for TargetId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "target:{}", self.0)
    }
}

/// Opaque identifier of a runtime generator.
///
/// A [`GeneratorId`] always carries its parent [`TargetId`]. This is the only
/// intentional semantic accessor exposed on the id; the rest of the
/// representation is private.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneratorId {
    target: TargetId,
    seq: u64,
}

impl GeneratorId {
    pub(crate) const fn new(target: TargetId, seq: u64) -> Self {
        Self { target, seq }
    }

    /// Recover the parent [`TargetId`] of this generator id.
    #[must_use]
    pub const fn target_id(self) -> TargetId {
        self.target
    }
}

impl Debug for GeneratorId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "GeneratorId({}/{})", self.target.0, self.seq)
    }
}

impl Display for GeneratorId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "generator:{}/{}", self.target.0, self.seq)
    }
}

/// Opaque class identifier used to group generators with similar service
/// requirements for scheduling purposes.
///
/// Class names are application-defined and have no scheduler semantics beyond
/// equality and weighting (configured separately by the runtime). The
/// underlying string representation is not exposed except via [`ClassId::as_str`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ClassId {
    inner: Arc<str>,
}

impl ClassId {
    /// Construct a [`ClassId`] from any string-like value.
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self { inner: name.into() }
    }

    /// Borrow the class name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Debug for ClassId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "ClassId({:?})", &*self.inner)
    }
}

impl Display for ClassId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "class:{}", &*self.inner)
    }
}
