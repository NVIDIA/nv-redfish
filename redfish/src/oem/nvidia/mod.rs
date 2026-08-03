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

//! Support of NVIDIA OEM Extensions to Redfish.
//!
//! The schemas are vendored verbatim from [NVIDIA/bmcweb] and exposed
//! here as `schema`, together with every wrapper built on them. That
//! one set covers every NVIDIA platform: the per-platform schema
//! directories upstream duplicate it and no implementation uses them,
//! so platform differences live in Rust as quirks instead.
//!
//! `oem-nvidia` compiles only the schemas belonging to the service
//! features that are already enabled -- the NVIDIA chassis schemas come
//! with `chassis`, the manager schemas with `managers`, and so on.
//! Families extending no standard service have their own `oem-nvidia-*`
//! feature. See `redfish/features.toml` for the mapping.
//!
//! [NVIDIA/bmcweb]: https://github.com/NVIDIA/bmcweb/tree/develop/redfish-core/schema/oem/nvidia/csdl

mod compiled_schema;

/// NVIDIA OEM Schema.
pub use compiled_schema::redfish as schema;

/// NVIDIA OEM chassis support.
#[cfg(feature = "chassis")]
pub mod cbc_chassis;

/// NVIDIA OEM computer system support.
#[cfg(feature = "computer-systems")]
pub mod computer_system;

#[cfg(feature = "chassis")]
#[doc(inline)]
pub use cbc_chassis::NvidiaCbcChassis;

#[cfg(feature = "computer-systems")]
#[doc(inline)]
pub use computer_system::NvidiaComputerSystem;
