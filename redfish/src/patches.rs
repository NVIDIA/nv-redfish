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

//! Sometimes Redfish implementations do not perfectly match the CSDL
//! specification. This module provides helpers to deal with that.

use crate::schema::redfish::resource::State as ResourceStateSchema;
use serde_json::Value;

#[cfg(feature = "chassis")]
use crate::schema::redfish::resource::LocationType as ResourceLocationTypeSchema;

/// Remove unsupported `Status.State` enum values from a resource payload.
///
/// Some BMCs return state values outside Redfish schema enum constraints
/// (for example `"Standby"`). This helper drops only invalid state values
/// and keeps all other payload fields unchanged.
#[must_use]
pub fn remove_invalid_resource_state(mut resource: Value) -> Value {
    if let Value::Object(ref mut obj) = resource {
        if let Some(Value::Object(ref mut status)) = obj.get_mut("Status") {
            let state_is_invalid = status
                .get("State")
                .is_some_and(|v| serde_json::from_value::<ResourceStateSchema>(v.clone()).is_err());
            if state_is_invalid {
                status.remove("State");
            }
        }
    }
    resource
}

/// Remove unsupported `Location.PartLocation.LocationType` enum values from a resource payload.
///
/// Some BMCs return state values outside Redfish schema enum constraints
/// (for example `"Unknown"`). This helper drops only invalid state values
/// and keeps all other payload fields unchanged.
#[cfg(feature = "chassis")]
#[must_use]
pub fn remove_invalid_resource_part_location_type(mut resource: Value) -> Value {
    if let Value::Object(ref mut obj) = resource {
        if let Some(Value::Object(ref mut location)) = obj.get_mut("Location") {
            if let Some(Value::Object(ref mut part_location)) = location.get_mut("PartLocation") {
                let location_type_is_invalid = part_location.get("LocationType").is_some_and(|v| {
                    serde_json::from_value::<ResourceLocationTypeSchema>(v.clone()).is_err()
                });
                if location_type_is_invalid {
                    part_location.remove("LocationType");
                }
            }
        }
    }
    resource
}
