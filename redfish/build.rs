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
use nv_redfish_csdl_compiler::features_manifest::FeaturesManifest;
use nv_redfish_schema::cargo_feature_enabled;
use nv_redfish_schema::oem_schema;
use nv_redfish_schema::out_dir;
use nv_redfish_schema::redfish_schema;
use nv_redfish_schema::rerun_for;
use nv_redfish_schema::run_with_big_stack;
use nv_redfish_schema::swordfish_schema;
use std::error::Error as StdError;
use std::fs::File;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    run_with_big_stack(run)
}

fn run() -> Result<(), Box<dyn StdError>> {
    let features_manifest = PathBuf::from("features.toml");
    let manifest = FeaturesManifest::read(&features_manifest)?;
    rerun_for([&features_manifest]);

    let redfish_csdl: [&str; 5] = [
        "Settings_v1.xml",
        "Message_v1.xml",
        "Resource_v1.xml",
        "ResolutionStep_v1.xml",
        "ActionInfo_v1.xml",
    ];

    // ================================================================================
    // Compile standard DMTF schema

    let target_features = manifest
        .all_features()
        .into_iter()
        .filter(|f| cargo_feature_enabled(f))
        .collect::<Vec<_>>();

    let out_dir = out_dir();
    let service_root: [&str; 1] = ["ServiceRoot_v1.xml"];
    let service_root_patterns = ["ServiceRoot.*.*"]
        .iter()
        .map(|v| v.parse())
        .collect::<Result<Vec<_>, _>>()
        .expect("must be successfuly parsed");
    let features = manifest.collect(&target_features);

    let csdls = redfish_csdl
        .iter()
        .copied()
        .chain(service_root.iter().copied())
        .map(redfish_schema)
        .chain(features.csdl_files.iter().map(|f| redfish_schema(f)))
        .chain(
            features
                .swordfish_csdl_files
                .iter()
                .map(|f| swordfish_schema(f)),
        )
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    rerun_for(&csdls);

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        include_root_patterns: features.root_patterns.into_iter().cloned().collect(),
        output: out_dir.join("redfish.rs"),
        csdls,
        entity_type_patterns: service_root_patterns
            .iter()
            .chain(features.patterns)
            .cloned()
            .collect(),
        rigid_array_patterns: features.rigid_array_patterns.into_iter().cloned().collect(),
    })?;

    // ================================================================================
    // Compile OEM-specific schemas

    let vendors = manifest
        .all_vendors()
        .into_iter()
        .filter(|v| cargo_feature_enabled(&format!("oem-{v}")))
        .collect::<Vec<_>>();

    for v in vendors {
        let vendor_features = manifest
            .all_vendor_features(v)
            .into_iter()
            .filter(|name| cargo_feature_enabled(name))
            .collect::<Vec<_>>();

        let output = out_dir.join(format!("oem-{v}.rs"));
        if vendor_features.is_empty() {
            // Just create empty output file:
            File::create(output)?;
            continue;
        }

        let (root_csdls, resolve_csdls, patterns) =
            manifest.collect_vendor_features(v, &vendor_features);

        let root_csdls = root_csdls
            .iter()
            .map(|f| oem_schema(v, f))
            .collect::<Vec<_>>();

        let resolve_csdls = redfish_csdl
            .iter()
            .copied()
            .map(redfish_schema)
            .chain(resolve_csdls.iter().map(|f| redfish_schema(f)))
            .collect::<Vec<_>>();

        rerun_for(root_csdls.iter().chain(resolve_csdls.iter()));

        process_command(&Commands::CompileOem {
            output,
            root_csdls,
            resolve_csdls,
            entity_type_patterns: patterns.into_iter().cloned().collect(),
            rigid_array_patterns: vec![],
        })?;
    }
    Ok(())
}
