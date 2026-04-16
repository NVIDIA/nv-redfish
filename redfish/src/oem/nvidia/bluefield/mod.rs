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

//! Support of NVIDIA Bluefield DPU OEM extensions to Redfish.

mod compiled_schema;

/// NVIDIA Bluefield OEM Schema.
pub use compiled_schema::redfish as schema;

/// NVIDIA Bluefield OEM computer system support.
#[cfg(feature = "computer-systems")]
pub mod nvidia_computer_system;

#[doc(inline)]
pub use nvidia_computer_system::NvidiaComputerSystem;
