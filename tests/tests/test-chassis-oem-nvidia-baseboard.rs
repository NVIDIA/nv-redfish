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
//! Integration tests for NVIDIA Baseboard CBC chassis OEM extension.

#![recursion_limit = "256"]

use nv_redfish::chassis::Chassis;
use nv_redfish::Error as RedfishError;
use nv_redfish::ServiceRoot;
use nv_redfish_core::ODataId;
use nv_redfish_tests::json_merge;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde_json::json;
use serde_json::Value;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::test;

const SERVICE_ROOT_DATA_TYPE: &str = "#ServiceRoot.v1_13_0.ServiceRoot";
const CHASSIS_COLLECTION_DATA_TYPE: &str = "#ChassisCollection.ChassisCollection";
const CHASSIS_DATA_TYPE: &str = "#Chassis.v1_22_0.Chassis";

#[test]
async fn oem_nvidia_baseboard_cbc_real_payload() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = chassis_ids();
    let chassis = chassis_member(
        &ids,
        json!({
            "Oem": {
                "Nvidia": {
                    ODATA_TYPE: "#NvidiaChassis.v1_4_0.NvidiaCBCChassis",
                    "ChassisPhysicalSlotNumber": 24,
                    "ComputeTrayIndex": 14,
                    "RevisionId": 2,
                    "TopologyId": 128
                }
            }
        }),
    );
    let chassis = get_chassis(bmc.clone(), &ids, chassis).await?;

    let oem = chassis.oem_nvidia_baseboard_cbc()?;
    assert_eq!(
        oem.chassis_physical_slot_number().map(|v| *v.inner()),
        Some(24)
    );
    assert_eq!(oem.compute_tray_index().map(|v| *v.inner()), Some(14));
    assert_eq!(oem.revision_id().map(|v| *v.inner()), Some(2));
    assert_eq!(oem.topology_id().map(|v| *v.inner()), Some(128));

    Ok(())
}

#[test]
async fn oem_nvidia_baseboard_cbc_missing_oem_returns_not_available(
) -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = chassis_ids();
    let chassis = get_chassis(bmc.clone(), &ids, chassis_member(&ids, json!({}))).await?;

    assert!(matches!(
        chassis.oem_nvidia_baseboard_cbc(),
        Err(RedfishError::NvidiaCbcChassisNotAvailable)
    ));

    Ok(())
}

#[test]
async fn oem_nvidia_baseboard_cbc_wrong_odata_type_returns_not_available(
) -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = chassis_ids();
    let chassis = chassis_member(
        &ids,
        json!({
            "Oem": {
                "Nvidia": {
                    ODATA_TYPE: "#NvidiaChassis.v1_4_0.NvidiaChassis",
                    "ChassisPhysicalSlotNumber": 24,
                    "ComputeTrayIndex": 14,
                    "RevisionId": 2,
                    "TopologyId": 128
                }
            }
        }),
    );
    let chassis = get_chassis(bmc.clone(), &ids, chassis).await?;

    assert!(matches!(
        chassis.oem_nvidia_baseboard_cbc(),
        Err(RedfishError::NvidiaCbcChassisNotAvailable)
    ));

    Ok(())
}

async fn get_chassis(
    bmc: Arc<Bmc>,
    ids: &ChassisIds,
    member: Value,
) -> Result<Chassis<Bmc>, Box<dyn StdError>> {
    let service_root = expect_service_root(bmc.clone(), ids).await?;
    let collection_name = resource_name(&ids.chassis_collection_id);
    bmc.expect(Expect::expand(
        &ids.chassis_collection_id,
        json!({
            ODATA_ID: &ids.chassis_collection_id,
            ODATA_TYPE: CHASSIS_COLLECTION_DATA_TYPE,
            "Id": collection_name,
            "Name": "Chassis Collection",
            "Members": [member]
        }),
    ));
    let collection = service_root.chassis().await?;
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);
    Ok(members
        .into_iter()
        .next()
        .expect("single chassis must exist"))
}

async fn expect_service_root(
    bmc: Arc<Bmc>,
    ids: &ChassisIds,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        json!({
            ODATA_ID: &ids.root_id,
            ODATA_TYPE: SERVICE_ROOT_DATA_TYPE,
            "Id": "RootService",
            "Name": "RootService",
            "ProtocolFeaturesSupported": {
                "ExpandQuery": {
                    "NoLinks": true
                }
            },
            "Chassis": { ODATA_ID: &ids.chassis_collection_id },
            "Links": {},
        }),
    ));

    ServiceRoot::new(bmc).await.map_err(Into::into)
}

struct ChassisIds {
    root_id: ODataId,
    chassis_collection_id: String,
    chassis_id: String,
}

fn chassis_ids() -> ChassisIds {
    let root_id = ODataId::service_root();
    let chassis_collection_id = format!("{root_id}/Chassis");
    let chassis_id = format!("{chassis_collection_id}/CBC_0");
    ChassisIds {
        root_id,
        chassis_collection_id,
        chassis_id,
    }
}

fn resource_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

fn chassis_member(ids: &ChassisIds, fields: Value) -> Value {
    let name = resource_name(&ids.chassis_id);
    let base = json!({
        ODATA_ID: &ids.chassis_id,
        ODATA_TYPE: CHASSIS_DATA_TYPE,
        "Id": name,
        "Name": name,
        "ChassisType": "Component",
        "Status": {
            "Health": "OK",
            "State": "Enabled"
        }
    });
    json_merge([&base, &fields])
}
