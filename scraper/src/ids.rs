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

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetId {
    raw: u64,
}

impl TargetId {
    pub(crate) fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub(crate) fn raw(self) -> u64 {
        self.raw
    }
}

impl fmt::Display for TargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "target #{}", self.raw)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeneratorId {
    target_id: TargetId,
    raw: u64,
}

impl GeneratorId {
    pub(crate) fn new(target_id: TargetId, raw: u64) -> Self {
        Self { target_id, raw }
    }

    pub fn target_id(self) -> TargetId {
        self.target_id
    }

    pub(crate) fn raw(self) -> u64 {
        self.raw
    }
}

impl fmt::Display for GeneratorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "generator #{}.{}", self.target_id.raw(), self.raw())
    }
}
