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
//! Integration tests for Lenovo ComputerSystem OEM support.

#![recursion_limit = "256"]

use nv_redfish::computer_system::ComputerSystem;
use nv_redfish::oem::lenovo::computer_system::FpMode;
use nv_redfish::oem::lenovo::computer_system::PortSwitchingTo;
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
const SYSTEM_COLLECTION_DATA_TYPE: &str = "#ComputerSystemCollection.ComputerSystemCollection";
const SYSTEM_DATA_TYPE: &str = "#ComputerSystem.v1_19_0.ComputerSystem";

#[test]
async fn lenovo_computer_system_usb_management_fields() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let system = get_system(
        bmc.clone(),
        &ids,
        system_payload(
            &ids,
            Some(json!({
                ODATA_TYPE: "#LenovoComputerSystem.v1_0_0.LenovoSystemProperties",
                "USBManagementPortAssignment": {
                    "FPMode": "Server",
                    "PortSwitchingTo": "Server"
                }
            })),
        ),
    )
    .await?;

    let lenovo = system.oem_lenovo()?;
    assert_eq!(lenovo.front_panel_mode(), Some(FpMode::Server));
    assert_eq!(lenovo.port_switching_to(), Some(PortSwitchingTo::Server));

    Ok(())
}

#[test]
async fn system_without_lenovo_oem_returns_not_available() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let system = get_system(bmc.clone(), &ids, system_payload(&ids, None)).await?;

    assert!(matches!(
        system.oem_lenovo(),
        Err(RedfishError::LenovoComputerSystemNotAvailable)
    ));

    Ok(())
}

async fn get_system(
    bmc: Arc<Bmc>,
    ids: &Ids,
    member: Value,
) -> Result<ComputerSystem<Bmc>, Box<dyn StdError>> {
    let root = expect_service_root(bmc.clone(), ids).await?;
    bmc.expect(Expect::expand(
        &ids.systems_id,
        json!({
            ODATA_ID: &ids.systems_id,
            ODATA_TYPE: SYSTEM_COLLECTION_DATA_TYPE,
            "Id": "Systems",
            "Name": "Computer System Collection",
            "Members": [member]
        }),
    ));

    let systems = root.systems().await?;
    let members = systems.members().await?;
    assert_eq!(members.len(), 1);
    Ok(members
        .into_iter()
        .next()
        .expect("single system must exist"))
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
            "Systems": { ODATA_ID: &ids.systems_id },
            "Links": {},
        }),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}

struct Ids {
    root_id: ODataId,
    systems_id: String,
    system_id: String,
}

fn ids() -> Ids {
    let root_id = ODataId::service_root();
    let systems_id = format!("{root_id}/Systems");
    let system_id = format!("{systems_id}/1");
    Ids {
        root_id,
        systems_id,
        system_id,
    }
}

fn system_payload(ids: &Ids, lenovo_oem: Option<Value>) -> Value {
    let base = json!({
        ODATA_ID: &ids.system_id,
        ODATA_TYPE: SYSTEM_DATA_TYPE,
        "Id": "1",
        "Name": "ComputerSystem",
        "Status": {
            "Health": "OK",
            "State": "Enabled"
        }
    });
    let oem = lenovo_oem.map_or_else(
        || json!({}),
        |lenovo| {
            json!({
                "Oem": {
                    "Lenovo": lenovo
                }
            })
        },
    );
    json_merge([&base, &oem])
}
