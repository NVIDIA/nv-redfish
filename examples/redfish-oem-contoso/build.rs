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

use nv_redfish_csdl_compiler::commands::process_command;
use nv_redfish_csdl_compiler::commands::Commands;
use nv_redfish_csdl_compiler::Error;
use glob::glob;
use std::env::var;
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let output = out_dir.join("redfish_oem_contoso.rs");

    let redfish_schemas = "../../redfish/schemas/redfish-csdl/*.xml";
    let oem_contoso_schemas = "../../redfish/schemas/oem/contoso/*.xml";

    let root_csdls = glob(oem_contoso_schemas)
        .unwrap()
        .filter_map(Result::ok)
        .map(|p| p.display().to_string())
        .collect();
    let resolve_csdls = glob(redfish_schemas)
        .unwrap()
        .filter_map(Result::ok)
        .map(|p| p.display().to_string())
        .collect();

    for f in &root_csdls {
        println!("cargo:rerun-if-changed={f}");
    }
    for f in &resolve_csdls {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::CompileOem {
        output,
        root_csdls,
        resolve_csdls,
        entity_type_patterns: ["ServiceRoot.*.*", "LogEntry.*"]
            .iter()
            .map(|v| v.parse())
            .collect::<Result<Vec<_>, _>>()
            .expect("must be successfuly parsed"),
    })?;
    Ok(())
}
