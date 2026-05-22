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

//! Bundled Redfish/Swordfish CSDL schemas plus shared helpers for nv-redfish
//! workspace build scripts.
//!
//! Path constants and resolvers point at the bundled CSDL files relative to
//! this crate's manifest directory. Build helpers consolidate the patterns
//! that are otherwise duplicated across every `build.rs` in the workspace.

mod build_helpers;
mod paths;

pub use build_helpers::{cargo_feature_enabled, out_dir, rerun_for, run_with_big_stack};
pub use paths::{
    glob_oem_xml, glob_redfish_xml, glob_swordfish_xml, oem_schema, redfish_schema,
    swordfish_schema, OEM_DIR, REDFISH_CSDL_DIR, SWORDFISH_CSDL_DIR,
};
