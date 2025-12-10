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

use std::env::var;
use std::path::PathBuf;

use nv_redfish_csdl_compiler::commands::{process_command, Commands, DEFAULT_ROOT};
use nv_redfish_csdl_compiler::Error;
use glob::glob;

fn main() -> Result<(), Error> {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let output = out_dir.join("redfish.rs");

    let redfish_schemas = "../../redfish/schemas/redfish-csdl/*.xml";
    let swordfish_schemas = "../../redfish/schemas/swordfish-csdl/*.xml";

    let mut csdls = Vec::new();
    csdls.extend(
        glob(redfish_schemas)
            .unwrap()
            .filter_map(Result::ok)
            .map(|p| p.display().to_string()),
    );
    csdls.extend(
        glob(swordfish_schemas)
            .unwrap()
            .filter_map(Result::ok)
            .map(|p| p.display().to_string()),
    );

    for f in &csdls {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        output,
        csdls,
        entity_type_patterns: [
            "ServiceRoot.*.*",
            "ChassisCollection.*",
            "Chassis.*",
            "AccountService.*",
            "ManagerAccountCollection.*",
            "ManagerAccount.*",
            "Bios.*",
            "ComputerSystemCollection.*",
            "ComputerSystem.*",
            "PCIeDeviceCollection.*",
            "PCIeDevice.*",
            "PCIeFunctionCollection.*",
            "PCIeFunction.*",
            "Thermal.*",
            "Thermal.*.*",
            "ThermalMetrics.*",
            "ThermalSubsystem.*",
            "Sensor.*",
        ]
        .iter()
        .map(|v| v.parse())
        .collect::<Result<Vec<_>, _>>()
        .expect("must be successfuly parsed"),
    })?;
    Ok(())
}
