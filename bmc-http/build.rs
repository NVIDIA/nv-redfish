// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use glob::glob;
use nv_redfish_csdl_compiler::commands::Commands;
use nv_redfish_csdl_compiler::commands::process_command;
use std::env::var;
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::path::PathBuf;

const REDFISH_ERROR_SCHEMA: &str = "RedfishError_v1.xml";
const REDFISH_SCHEMA_DIR: &str = "../redfish/schemas/redfish-csdl/csdl";

fn main() -> Result<(), Box<dyn StdError>> {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_REQWEST");
    if var("CARGO_FEATURE_REQWEST").is_err() {
        return Ok(());
    }

    let out_dir = PathBuf::from(var("OUT_DIR")?);
    let output = out_dir.join("redfish.rs");
    let redfish_error_schema_name = Some(OsStr::new(REDFISH_ERROR_SCHEMA));

    let root_csdls = vec![redfish_schema(REDFISH_ERROR_SCHEMA)];
    let resolve_csdls = glob(&redfish_schema("*.xml"))?
        .filter_map(Result::ok)
        .filter(|path| path.file_name() != redfish_error_schema_name)
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();

    for f in root_csdls.iter().chain(resolve_csdls.iter()) {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::CompileOem {
        output,
        root_csdls,
        resolve_csdls,
        entity_type_patterns: Vec::new(),
        rigid_array_patterns: Vec::new(),
    })?;

    Ok(())
}

fn redfish_schema(file: &str) -> String {
    format!("{REDFISH_SCHEMA_DIR}/{file}")
}
