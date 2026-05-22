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

//! Path constants and resolvers for the bundled CSDL schemas.
//!
//! Constants resolve to absolute paths inside this crate's source tree at the
//! time the crate itself is compiled (via `env!("CARGO_MANIFEST_DIR")`). When
//! consumed as a `[build-dependencies]`, they point at the unpacked schemas
//! inside `~/.cargo/registry/src/.../nv-redfish-schema-<ver>/` for published
//! builds, or at the in-tree submodule checkout for in-workspace builds.

/// Directory holding the bundled DMTF Redfish CSDL schemas.
///
/// Mirrors the `csdl` subdirectory of the upstream
/// [`DMTF/Redfish-Publications`](https://github.com/DMTF/Redfish-Publications)
/// git submodule.
pub const REDFISH_CSDL_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/redfish-csdl/csdl");

/// Directory holding the bundled SNIA Swordfish CSDL schemas.
///
/// Mirrors the `csdl-schema` subdirectory of the upstream
/// [`SNIA/Swordfish-Publications`](https://github.com/SNIA/Swordfish-Publications)
/// git submodule.
pub const SWORDFISH_CSDL_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/swordfish-csdl/csdl-schema");

/// Directory holding the nv-redfish workspace's own OEM CSDL schemas.
pub const OEM_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/oem");

/// Resolve `name` (e.g. `"RedfishError_v1.xml"`) to an absolute path inside
/// [`REDFISH_CSDL_DIR`].
#[must_use]
pub fn redfish_schema(name: &str) -> String {
    format!("{REDFISH_CSDL_DIR}/{name}")
}

/// Resolve `name` to an absolute path inside [`SWORDFISH_CSDL_DIR`].
#[must_use]
pub fn swordfish_schema(name: &str) -> String {
    format!("{SWORDFISH_CSDL_DIR}/{name}")
}

/// Resolve `name` inside the OEM directory `vendor` (e.g. `oem_schema("dell",
/// "DellAttributes_v1.xml")`) to an absolute path inside [`OEM_DIR`].
#[must_use]
pub fn oem_schema(vendor: &str, name: &str) -> String {
    format!("{OEM_DIR}/{vendor}/{name}")
}

/// Return absolute paths for every `*.xml` file directly inside
/// [`REDFISH_CSDL_DIR`].
#[must_use]
pub fn glob_redfish_xml() -> Vec<String> {
    glob_xml(REDFISH_CSDL_DIR)
}

/// Return absolute paths for every `*.xml` file directly inside
/// [`SWORDFISH_CSDL_DIR`].
#[must_use]
pub fn glob_swordfish_xml() -> Vec<String> {
    glob_xml(SWORDFISH_CSDL_DIR)
}

/// Return absolute paths for every `*.xml` file directly inside the OEM
/// directory `vendor`.
#[must_use]
pub fn glob_oem_xml(vendor: &str) -> Vec<String> {
    glob_xml(&format!("{OEM_DIR}/{vendor}"))
}

fn glob_xml(dir: &str) -> Vec<String> {
    glob::glob(&format!("{dir}/*.xml"))
        .expect("invalid glob pattern for bundled schemas")
        .filter_map(Result::ok)
        .map(|p| p.display().to_string())
        .collect()
}
