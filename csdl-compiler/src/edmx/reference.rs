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

use serde::Deserialize;

use crate::ValidateError;
use crate::edmx::include::Include;
use crate::edmx::include_annotations::IncludeAnnotations;

/// 3.3 Element edmx:Reference
#[derive(Debug, Deserialize)]
pub struct DeReference {
    #[serde(rename = "@Uri")]
    pub uri: String,
    /// Child elements of Edmx.
    #[serde(rename = "$value", default)]
    pub items: Vec<DeReferenceItem>,
}

/// Child elements of `edmx::Reference`
#[derive(Debug, Deserialize)]
pub enum DeReferenceItem {
    Include(Include),
    IncludeAnnotations(IncludeAnnotations),
}

/// Validated reference stuct
pub struct Reference {
    pub uri: String,
    pub includes: Vec<Include>,
    pub include_annotations: Vec<IncludeAnnotations>,
}

impl DeReference {
    /// # Errors
    ///
    /// Actually, never returns error today but keep validation consistent.
    pub fn validate(self) -> Result<Reference, ValidateError> {
        let (includes, include_annotations) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut is, mut ias), v| {
                    match v {
                        DeReferenceItem::Include(v) => is.push(v),
                        DeReferenceItem::IncludeAnnotations(v) => ias.push(v),
                    }
                    (is, ias)
                });

        Ok(Reference {
            uri: self.uri,
            includes,
            include_annotations,
        })
    }
}
