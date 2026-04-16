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

//! Redfish protocol features

use crate::schema::service_root::Expand;
use crate::schema::service_root::ProtocolFeaturesSupported;
use std::convert::identity;

/// Defines features supported by Redfish protocol. Provides helpers
/// to write code that takes features in account.
#[derive(Default)]
pub struct ProtocolFeatures {
    /// Expand query features support.
    pub expand: ExpandQueryFeatures,
}

impl ProtocolFeatures {
    /// Create protocol features from deserialized structure.
    pub(crate) fn new(f: &ProtocolFeaturesSupported) -> Self {
        Self {
            expand: f
                .expand_query
                .as_ref()
                .map(ExpandQueryFeatures::new)
                .unwrap_or_default(),
        }
    }
}

/// Expand query support.
pub struct ExpandQueryFeatures {
    /// Indicates '*' support by the Server.
    pub expand_all: bool,
    /// Indicates '.' support by the Server.
    pub no_links: bool,
}

// We want to have explicit defaults. Not language one. They are the
// same by coincidence.
#[allow(clippy::derivable_impls)]
impl Default for ExpandQueryFeatures {
    fn default() -> Self {
        Self {
            expand_all: false,
            no_links: false,
        }
    }
}

impl ExpandQueryFeatures {
    pub fn new(f: &Expand) -> Self {
        Self {
            expand_all: f.expand_all.is_some_and(identity),
            no_links: f.no_links.is_some_and(identity),
        }
    }
}
