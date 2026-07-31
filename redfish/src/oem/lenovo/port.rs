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

//! Lenovo network port OEM extension.

use crate::schema::port::Port as PortSchema;
use serde_json::Value;

/// Borrowed view of Lenovo's OEM Port attributes.
pub struct LenovoPort<'a> {
    data: &'a Value,
}

impl<'a> LenovoPort<'a> {
    /// Get the Lenovo OEM view when `Oem.Lenovo` is present.
    pub fn new(port: &'a PortSchema) -> Option<Self> {
        port.base
            .base
            .oem
            .as_ref()
            .and_then(|oem| oem.additional_properties.get("Lenovo"))
            .map(|data| Self { data })
    }

    /// Get Lenovo's physical-port MAC address.
    #[must_use]
    pub fn physical_port_mac_address(&self) -> Option<&'a str> {
        self.data
            .get("PhysicalPortMacAddress")
            .and_then(Value::as_str)
    }
}
