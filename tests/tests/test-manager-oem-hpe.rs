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
//! Integration tests for HPE Manager OEM support.

#![recursion_limit = "256"]

use nv_redfish::manager::Manager;
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
const MANAGER_COLLECTION_DATA_TYPE: &str = "#ManagerCollection.ManagerCollection";
const MANAGER_DATA_TYPE: &str = "#Manager.v1_16_0.Manager";
const HPE_ILO_DATA_TYPE: &str = "#HpeiLO.v2_11_0.HpeiLO";

#[test]
async fn hpe_virtual_nic_enabled_supported() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(
        bmc.clone(),
        &ids,
        manager_payload(&ids, Some(json!(true))),
    )
    .await?;

    let hpe = manager.oem_hpe()?.unwrap();
    assert_eq!(hpe.virtual_nic_enabled(), Some(true));

    Ok(())
}

#[test]
async fn manager_without_hpe_oem_returns_none() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(bmc.clone(), &ids, manager_payload_without_hpe(&ids)).await?;

    assert!(manager.oem_hpe()?.is_none());

    Ok(())
}

#[test]
async fn malformed_hpe_oem_returns_parse_error() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(
        bmc.clone(),
        &ids,
        manager_payload(&ids, Some(json!("true"))),
    )
    .await?;

    let err = match manager.oem_hpe() {
        Ok(v) => panic!("expected parse error, got: {:?}", v.is_some()),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("invalid type"),
        "unexpected error: {}",
        err
    );

    Ok(())
}

async fn get_manager(
    bmc: Arc<Bmc>,
    ids: &Ids,
    manager: Value,
) -> Result<Manager<Bmc>, Box<dyn StdError>> {
    let root = expect_service_root(bmc.clone(), ids).await?;
    bmc.expect(Expect::expand(
        &ids.managers_id,
        json!({
            ODATA_ID: &ids.managers_id,
            ODATA_TYPE: MANAGER_COLLECTION_DATA_TYPE,
            "Id": "Managers",
            "Name": "Manager Collection",
            "Members": [manager]
        }),
    ));

    let collection = root.managers().await?.unwrap();
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);
    Ok(members
        .into_iter()
        .next()
        .expect("single manager must exist"))
}

async fn expect_service_root(
    bmc: Arc<Bmc>,
    ids: &Ids,
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
            "Managers": { ODATA_ID: &ids.managers_id },
            "Links": {},
        }),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}

struct Ids {
    root_id: ODataId,
    managers_id: String,
    manager_id: String,
}

fn ids() -> Ids {
    let root_id = ODataId::service_root();
    let managers_id = format!("{root_id}/Managers");
    let manager_id = format!("{managers_id}/1");
    Ids {
        root_id,
        managers_id,
        manager_id,
    }
}

fn manager_payload(ids: &Ids, virtual_nic_enabled: Option<Value>) -> Value {
    let base = json!({
        ODATA_ID: &ids.manager_id,
        ODATA_TYPE: MANAGER_DATA_TYPE,
        "Id": "1",
        "Name": "Manager",
        "ManagerType": "BMC",
        "Status": { "State": "Enabled" },
    });

    let mut hpe = json!({
        ODATA_TYPE: HPE_ILO_DATA_TYPE,
    });
    if let Some(v) = virtual_nic_enabled {
        hpe["VirtualNICEnabled"] = v;
    }

    let oem = json!({
        "Oem": {
            "Hpe": hpe
        }
    });
    json_merge([&base, &oem])
}

fn manager_payload_without_hpe(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.manager_id,
        ODATA_TYPE: MANAGER_DATA_TYPE,
        "Id": "1",
        "Name": "Manager",
        "ManagerType": "BMC",
        "Status": { "State": "Enabled" },
        "Oem": {}
    })
}
