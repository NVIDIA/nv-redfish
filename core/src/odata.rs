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

//! OData identifiers used by generated types
//!
//! Minimal wrappers for Redfish/OData identifiers used throughout generated code:
//! - [`ODataId`]: value of `@odata.id`, the canonical resource path (opaque string)
//! - [`ODataETag`]: value of `@odata.etag`, the HTTP entity tag (opaque string)
//!
//! Notes
//! - These types are intentionally semantic‑unaware; they do not validate content.
//! - [`ODataId::service_root()`] returns the conventional Redfish service root path.
//! - Formatting/Display returns the raw underlying string.
//!
//! Example
//! ```rust
//! use nv_redfish_core::ODataId;
//!
//! let root = ODataId::service_root();
//! assert_eq!(root.to_string(), "/redfish/v1");
//! ```
//!
//! References:
//! - OASIS OData 4.01 — `@odata.id`, `@odata.etag`
//! - DMTF Redfish Specification DSP0266 — `https://www.dmtf.org/standards/redfish`
//!

use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use serde::Deserialize;
use serde::Serialize;

/// Type for `@odata.id` identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ODataId(String);

impl ODataId {
    /// Redfish service root id.
    #[must_use]
    pub fn service_root() -> Self {
        Self("/redfish/v1".into())
    }
}

impl From<String> for ODataId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl Display for ODataId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

/// Type for `@odata.etag` identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ODataETag(String);

impl From<String> for ODataETag {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Display for ODataETag {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

/// Type for retrieving `@odata.type` from a JSON payload.
pub struct ODataType<'a> {
    /// Namespace of the data type. For example: `["Chassis", "v1_22_0"]`.
    pub namespace: Vec<&'a str>,
    /// Name of the type. For example "Chassis".
    pub type_name: &'a str,
}

impl ODataType<'_> {
    /// Get `@odata.type` from a JSON payload and parse it.
    #[must_use]
    pub fn parse_from(v: &serde_json::Value) -> Option<ODataType<'_>> {
        v.get("@odata.type")
            .and_then(|v| v.as_str())
            .and_then(|v| v.starts_with('#').then_some(&v[1..]))
            .and_then(|v| {
                let mut all = v.split('.').collect::<Vec<_>>();
                all.pop().map(|type_name| ODataType {
                    namespace: all,
                    type_name,
                })
            })
    }
}
