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
//! NVIDIA ships two independent OEM schema sets. They both define the
//! `NvidiaComputerSystem` namespace with incompatible shapes, so they
//! cannot share a compile unit and are exposed as separate vendors,
//! each behind its own feature with its own `schema`.
//!
//! `baseboard` is vendored verbatim from [NVIDIA/bmcweb] and compiles
//! only the schemas belonging to the service features already enabled --
//! the NVIDIA chassis schemas come with `chassis`, the manager schemas
//! with `managers`, and so on. Families extending no standard service
//! have their own `oem-nvidia-*` feature. See `redfish/features.toml`.
//!
//! `bluefield` covers the DPU and is maintained by hand.
//!
//! [NVIDIA/bmcweb]: https://github.com/NVIDIA/bmcweb/tree/develop/redfish-core/schema/oem/nvidia/csdl

#[cfg(feature = "oem-nvidia-bluefield")]
pub mod bluefield;

#[cfg(feature = "oem-nvidia-baseboard")]
pub mod baseboard;
