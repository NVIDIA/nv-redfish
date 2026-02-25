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
//! Integration tests for Chassis collection workaround behavior.

#![recursion_limit = "256"]

use nv_redfish::ServiceRoot;
use nv_redfish_core::ODataId;
use nv_redfish_tests::ami_viking_service_root;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde_json::json;
use serde_json::Value;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::test;

const CHASSIS_COLLECTION_DATA_TYPE: &str = "#ChassisCollection.ChassisCollection";
const CHASSIS_DATA_TYPE: &str = "#Chassis.v1_23_0.Chassis";

#[test]
async fn ami_viking_missing_root_chassis_nav_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let root = expect_viking_service_root(bmc.clone(), &ids, json!({})).await?;
    expect_chassis_collection(bmc.clone(), &ids);

    let collection = root.chassis().await?.unwrap();
    expect_chassis_get(bmc.clone(), &ids, valid_chassis_payload(&ids));
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

#[test]
async fn ami_viking_invalid_contained_by_fields_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let root = expect_viking_service_root(
        bmc.clone(),
        &ids,
        json!({
            "Chassis": { ODATA_ID: &ids.chassis_collection_id }
        }),
    )
    .await?;
    expect_chassis_collection(bmc.clone(), &ids);

    let collection = root.chassis().await?.unwrap();
    expect_chassis_get(
        bmc.clone(),
        &ids,
        json!({
            ODATA_ID: &ids.chassis_id,
            ODATA_TYPE: CHASSIS_DATA_TYPE,
            "Id": "1",
            "Name": "Chassis",
            "ChassisType": "RackMount",
            "Links": {
                "ContainedBy": {
                    ODATA_ID: &ids.container_chassis_id,
                    "InvalidField": "invalid"
                }
            }
        }),
    );
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

#[test]
async fn ami_viking_missing_chassis_type_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let root = expect_viking_service_root(
        bmc.clone(),
        &ids,
        json!({
            "Chassis": { ODATA_ID: &ids.chassis_collection_id }
        }),
    )
    .await?;
    expect_chassis_collection(bmc.clone(), &ids);

    let collection = root.chassis().await?.unwrap();
    expect_chassis_get(
        bmc.clone(),
        &ids,
        json!({
            ODATA_ID: &ids.chassis_id,
            ODATA_TYPE: CHASSIS_DATA_TYPE,
            "Id": "1",
            "Name": "Chassis"
        }),
    );
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

#[test]
async fn ami_viking_missing_chassis_name_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let root = expect_viking_service_root(
        bmc.clone(),
        &ids,
        json!({
            "Chassis": { ODATA_ID: &ids.chassis_collection_id }
        }),
    )
    .await?;
    expect_chassis_collection(bmc.clone(), &ids);

    let collection = root.chassis().await?.unwrap();
    expect_chassis_get(
        bmc.clone(),
        &ids,
        json!({
            ODATA_ID: &ids.chassis_id,
            ODATA_TYPE: CHASSIS_DATA_TYPE,
            "Id": "1",
            "ChassisType": "RackMount"
        }),
    );
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

async fn expect_viking_service_root(
    bmc: Arc<Bmc>,
    ids: &Ids,
    fields: Value,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        ami_viking_service_root(&ids.root_id, fields),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}

fn expect_chassis_collection(bmc: Arc<Bmc>, ids: &Ids) {
    bmc.expect(Expect::get(
        &ids.chassis_collection_id,
        json!({
            ODATA_ID: &ids.chassis_collection_id,
            ODATA_TYPE: CHASSIS_COLLECTION_DATA_TYPE,
            "Id": "Chassis",
            "Name": "Chassis Collection",
            "Members": [
                {
                    ODATA_ID: &ids.chassis_id
                }
            ]
        }),
    ));
}

fn expect_chassis_get(bmc: Arc<Bmc>, ids: &Ids, payload: Value) {
    bmc.expect(Expect::get(&ids.chassis_id, payload));
}

fn valid_chassis_payload(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.chassis_id,
        ODATA_TYPE: CHASSIS_DATA_TYPE,
        "Id": "1",
        "Name": "Chassis",
        "ChassisType": "RackMount"
    })
}

struct Ids {
    root_id: ODataId,
    chassis_collection_id: String,
    chassis_id: String,
    container_chassis_id: String,
}

fn ids() -> Ids {
    let root_id = ODataId::service_root();
    let chassis_collection_id = format!("{root_id}/Chassis");
    let chassis_id = format!("{chassis_collection_id}/1");
    let container_chassis_id = format!("{chassis_collection_id}/0");
    Ids {
        root_id,
        chassis_collection_id,
        chassis_id,
        container_chassis_id,
    }
}
