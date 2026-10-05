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

//! Support of HPE OEM extensions to Redfish.

#[cfg(feature = "accounts")]
pub mod account_service;

#[cfg(all(feature = "bios", feature = "computer-systems"))]
pub mod bios;

#[cfg(feature = "computer-systems")]
pub mod computer_system_actions;

#[cfg(feature = "managers")]
pub mod date_time;

#[cfg(feature = "managers")]
pub mod manager;

#[cfg(all(feature = "manager-network-protocol", feature = "managers"))]
pub mod manager_network_protocol;

pub mod ilo_service_ext;

#[cfg(feature = "bios")]
pub mod server_boot_settings;

#[cfg(any(feature = "accounts", feature = "managers"))]
mod update;

#[cfg(feature = "accounts")]
#[doc(inline)]
pub use account_service::{HpeAccountServiceUpdate, HpeAccountServiceUpdateExt};
#[cfg(all(feature = "bios", feature = "computer-systems"))]
#[doc(inline)]
pub use bios::HpeBios;
#[cfg(feature = "computer-systems")]
#[doc(inline)]
pub use computer_system_actions::{HpeComputerSystemActions, ResetType as HpeSystemResetType};
#[cfg(feature = "managers")]
#[doc(inline)]
pub use date_time::HpeiLoDateTime;
#[cfg(feature = "managers")]
#[doc(inline)]
pub use manager::HpeManager;
#[cfg(all(feature = "manager-network-protocol", feature = "managers"))]
#[doc(inline)]
pub use manager_network_protocol::HpeManagerNetworkProtocol;

#[doc(inline)]
pub use ilo_service_ext::HpeiLoServiceExt;

#[cfg(feature = "bios")]
#[doc(inline)]
pub use server_boot_settings::HpeServerBootSettings;

mod compiled_schema;

/// HPE OEM schema.
pub use compiled_schema::redfish as schema;
