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

//! Support of Delta Energy Systems OEM extensions to Redfish.

mod compiled_schema;

/// Delta Energy Systems OEM Schema.
pub use compiled_schema::redfish as schema;

/// Support of Delta OEM `PowerSupply` extensions.
#[cfg(feature = "power-supplies")]
pub mod power_supply;

#[cfg(feature = "power-supplies")]
#[doc(inline)]
pub use power_supply::DeltaPowerSupply;

#[cfg(feature = "chassis")]
use crate::chassis;

/// Manufacturer reported in the chassis collection member of a Delta power shelf.
#[cfg(feature = "chassis")]
pub const CHASSIS_MANUFACTURER: chassis::Manufacturer<&'static str> =
    chassis::Manufacturer::new("DELTA");
