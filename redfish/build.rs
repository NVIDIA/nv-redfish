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
use nv_redfish_csdl_compiler::features_manifest::FeaturesManifest;
use std::env::var;
use std::error::Error as StdError;
use std::fs::File;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn StdError>> {
    let features_manifest = PathBuf::from("features.toml");
    let manifest = FeaturesManifest::read(&features_manifest)?;
    println!("cargo:rerun-if-changed={}", features_manifest.display());

    let redfish_csdl = vec![
        "Settings_v1.xml",
        "Message_v1.xml",
        "Resource_v1.xml",
        "ResolutionStep_v1.xml",
        "ActionInfo_v1.xml",
    ]
    .into_iter()
    .map(Into::into)
    .collect::<Vec<String>>();

    // ================================================================================
    // Compile standard DMTF schema

    // Collect features that is defined by configuration
    let target_features = manifest
        .all_features()
        .into_iter()
        .filter(|f| {
            var(format!(
                "CARGO_FEATURE_{}",
                f.to_uppercase().replace('-', "_")
            ))
            .is_ok()
        })
        .collect::<Vec<_>>();

    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let output = out_dir.join("redfish.rs");
    let redfish_schema_path = "schemas/redfish-csdl/csdl";
    let swordfish_schema_path = "schemas/swordfish-csdl/csdl-schema";
    let service_root = vec!["ServiceRoot_v1.xml"]
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    let service_root_pattens = vec!["ServiceRoot.*.*"]
        .into_iter()
        .map(|v| v.parse())
        .collect::<Result<Vec<_>, _>>()
        .expect("must be successfuly parsed");
    let (features_csdls, features_swordfish_csdls, features_patterns) =
        manifest.collect(&target_features);
    let csdls = redfish_csdl
        .iter()
        .chain(service_root.iter())
        .chain(features_csdls)
        .map(|f| format!("{redfish_schema_path}/{f}"))
        .chain(
            features_swordfish_csdls
                .iter()
                .map(|f| format!("{swordfish_schema_path}/{f}")),
        )
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    for f in &csdls {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        output,
        csdls,
        entity_type_patterns: service_root_pattens
            .iter()
            .chain(features_patterns)
            .cloned()
            .collect(),
    })?;

    // ================================================================================
    // Compile OEM-specific schemas

    let vendors = manifest
        .all_vendors()
        .into_iter()
        .filter(|v| {
            var(format!(
                "CARGO_FEATURE_OEM_{}",
                v.to_uppercase().replace('-', "_")
            ))
            .is_ok()
        })
        .collect::<Vec<_>>();

    for v in vendors {
        let features = manifest
            .all_vendor_features(v)
            .into_iter()
            .filter(|v| {
                var(format!(
                    "CARGO_FEATURE_{}",
                    v.to_uppercase().replace('-', "_")
                ))
                .is_ok()
            })
            .collect::<Vec<_>>();

        let output = out_dir.join(format!("oem-{v}.rs"));
        if features.is_empty() {
            // Just create empty output file:
            File::create(output)?;
            continue;
        }

        let (root_csdls, resolve_csdls, patterns) = manifest.collect_vendor_features(v, &features);
        let oem_schema_path = "schemas/oem";

        let root_csdls = root_csdls
            .iter()
            .map(|f| format!("{oem_schema_path}/{v}/{f}"))
            .collect::<Vec<_>>();

        let resolve_csdls = redfish_csdl
            .iter()
            .chain(resolve_csdls.into_iter())
            .map(|f| format!("{redfish_schema_path}/{f}"))
            .collect::<Vec<_>>();

        for f in root_csdls.iter().chain(resolve_csdls.iter()) {
            println!("cargo:rerun-if-changed={f}");
        }

        process_command(&Commands::CompileOem {
            output,
            root_csdls,
            resolve_csdls,
            entity_type_patterns: patterns.into_iter().cloned().collect(),
        })?;
    }
    Ok(())
}
