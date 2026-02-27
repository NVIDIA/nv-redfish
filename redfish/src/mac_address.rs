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

/// MAC address returned by the crate.
///
/// nv-redfish is not opionated about format of the MAC addresses. So,
/// it returns whatever server returns. This type is only introduced
/// to reduce number of untyped &str returned by functions.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct MacAddress<'a>(&'a str);

impl MacAddress<'_> {
    /// Create new MAC-address.
    #[must_use]
    pub const fn new(v: &str) -> MacAddress<'_> {
        MacAddress(v)
    }

    /// String representation MAC-address.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0
    }
}

impl fmt::Display for MacAddress<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
