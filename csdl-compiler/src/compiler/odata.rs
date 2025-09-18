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

//! `OData` related attributes needed to generate code.

use crate::odata::annotations::DescriptionRef;
use crate::odata::annotations::LongDescriptionRef;
use crate::odata::annotations::ODataAnnotations;
use tagged_types::TaggedType;

pub type MustHaveId = TaggedType<bool, MustHaveIdTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy)]
#[transparent(Debug)]
#[capability(inner_access)]
pub enum MustHaveIdTag {}

/// `OData` attributes attached to different compiled enities.
#[derive(Debug)]
pub struct CompiledOData<'a> {
    pub must_have_id: MustHaveId,
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

impl<'a> CompiledOData<'a> {
    /// Create new instance from reference to object that implements
    /// annotations.
    pub fn new(must_have_id: MustHaveId, src: &'a impl ODataAnnotations) -> Self {
        Self {
            must_have_id,
            description: src.odata_description(),
            long_description: src.odata_long_description(),
        }
    }

    /// `OData` doesn't contain anything.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.description.is_none() && self.long_description.is_none()
    }
}
