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

//! Redfish-related attributes needed to generate code.

use crate::redfish::annotations::IsRequired;
use crate::redfish::annotations::IsRequiredOnCreate;
use crate::redfish::annotations::RedfishPropertyAnnotations;

/// Redfish property attributes attached to different compiled enities.
#[derive(Debug)]
pub struct RedfishProperty {
    pub is_required: IsRequired,
    pub is_required_on_create: IsRequiredOnCreate,
}

impl RedfishProperty {
    /// Create new instance from reference to object that implements
    /// annotations.
    pub fn new(src: &impl RedfishPropertyAnnotations) -> Self {
        Self {
            is_required: src.is_required(),
            is_required_on_create: src.is_required_on_create(),
        }
    }
}
