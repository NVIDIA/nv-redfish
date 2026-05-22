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

use nv_redfish_csdl_compiler::commands::process_command;
use nv_redfish_csdl_compiler::commands::Commands;
use nv_redfish_csdl_compiler::commands::DEFAULT_ROOT;
use nv_redfish_csdl_compiler::Error;
use nv_redfish_schema::glob_redfish_xml;
use nv_redfish_schema::glob_swordfish_xml;
use nv_redfish_schema::out_dir;
use nv_redfish_schema::rerun_for;
use nv_redfish_schema::run_with_big_stack;

fn main() -> Result<(), String> {
    run_with_big_stack(run)
}

fn run() -> Result<(), Error> {
    // Swordfish contains some entities that are also defined in Redfish; the
    // Redfish ones take precedence so the Swordfish duplicates are filtered out.
    const SWORDFISH_REDFISH_ENTITIES: &[&str] = &[
        "DriveCollection_v1.xml",
        "EndpointCollection_v1.xml",
        "EndpointGroupCollection_v1.xml",
        "EndpointGroup_v1.xml",
        "Endpoint_v1.xml",
        "Schedule_v1.xml",
        "ServiceRoot_v1.xml",
        "VolumeCollection_v1.xml",
        "Volume_v1.xml",
    ];

    let mut csdls = glob_redfish_xml();
    csdls.extend(
        glob_swordfish_xml()
            .into_iter()
            .filter(|p| !SWORDFISH_REDFISH_ENTITIES.iter().any(|e| p.ends_with(e))),
    );

    rerun_for(&csdls);

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        include_root_patterns: [
            "Event.v1_0_0.EventRecord",
            "MetricReport.v1_0_0.MetricReport",
        ]
        .iter()
        .map(|v| v.parse())
        .collect::<Result<Vec<_>, _>>()
        .expect("must be successfuly parsed"),
        output: out_dir().join("redfish.rs"),
        csdls,
        entity_type_patterns: [
            "ServiceRoot.*.*",
            "ChassisCollection.*",
            "Chassis.*",
            "AccountService.*",
            "Event.*",
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
        rigid_array_patterns: vec![],
    })?;
    Ok(())
}
