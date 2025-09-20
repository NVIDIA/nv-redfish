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

use csdl_compiler::commands::Commands;
use csdl_compiler::commands::DEFAULT_ROOT;
use csdl_compiler::commands::Error;
use csdl_compiler::commands::process_command;
use glob::glob;
use std::env::var;
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let output = out_dir.join("redfish.rs");

    let redfish_schemas = "../../schemas/redfish-csdl/*.xml";
    let swordfish_schemas = "../../schemas/swordfish-csdl/*.xml";

    let mut csdls = Vec::new();
    csdls.extend(glob(redfish_schemas).unwrap().filter_map(Result::ok));
    csdls.extend(glob(swordfish_schemas).unwrap().filter_map(Result::ok));

    for f in &csdls {
        println!("cargo:rerun-if-changed={}", f.display());
    }

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        output,
        csdls,
    })?;
    Ok(())
}
