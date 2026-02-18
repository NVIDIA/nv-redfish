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
use nv_redfish_csdl_compiler::commands::DEFAULT_ROOT;
use nv_redfish_csdl_compiler::Error;
use std::env::var;
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());

    let base_csdls = ["./schemas/base/schema.xml"]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let base_output = out_dir.join("base_tests.rs");

    let all_csdls = base_csdls.iter();

    for f in all_csdls {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        output: base_output,
        csdls: base_csdls,
        entity_type_patterns: vec![],
        include_root_patterns: vec![],
    })?;
    Ok(())
}
