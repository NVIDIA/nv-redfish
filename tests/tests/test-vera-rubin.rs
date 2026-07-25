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
//! Vera Rubin (VR NVL72) boot-order integration tests.

use nv_redfish::computer_system::ComputerSystem;
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

const SERVICE_ROOT_DATA_TYPE: &str = "#ServiceRoot.v1_15_0.ServiceRoot";
const SYSTEM_COLLECTION_DATA_TYPE: &str = "#ComputerSystemCollection.ComputerSystemCollection";
const SYSTEM_DATA_TYPE: &str = "#ComputerSystem.v1_22_0.ComputerSystem";

#[test]
async fn vera_rubin_composite_boot_order_matches_boot_options() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let boot_options_id = format!("{}/BootOptions", ids.system_0_id);
    let boot0019_id = format!("{boot_options_id}/Boot0019");
    let boot0010_id = format!("{boot_options_id}/Boot0010");

    let system = get_system_0(
        bmc.clone(),
        &ids,
        json!({
            "Boot": {
                "BootOptions": { ODATA_ID: &boot_options_id },
                "BootOrder": [
                    "Boot0019: Ubuntu",
                    "Boot0010: UEFI HTTPv4 (MAC:F4204D494ECC)"
                ]
            }
        }),
    )
    .await?;

    bmc.expect(Expect::expand(
        &boot_options_id,
        json!({
            ODATA_ID: &boot_options_id,
            ODATA_TYPE: "#BootOptionCollection.BootOptionCollection",
            "Name": "Boot Option Collection",
            "Members": [
                { ODATA_ID: &boot0019_id },
                { ODATA_ID: &boot0010_id }
            ]
        }),
    ));
    bmc.expect(Expect::get(
        &boot0019_id,
        json!({
            ODATA_ID: &boot0019_id,
            ODATA_TYPE: "#BootOption.v1_0_4.BootOption",
            "Id": "Boot0019",
            "Name": "Boot0019",
            "BootOptionReference": "Boot0019",
            "DisplayName": "Ubuntu",
            "BootOptionEnabled": true
        }),
    ));
    bmc.expect(Expect::get(
        &boot0010_id,
        json!({
            ODATA_ID: &boot0010_id,
            ODATA_TYPE: "#BootOption.v1_0_4.BootOption",
            "Id": "Boot0010",
            "Name": "Boot0010",
            "BootOptionReference": "Boot0010",
            "DisplayName": "UEFI HTTPv4 (MAC:F4204D494ECC)",
            "BootOptionEnabled": true
        }),
    ));

    let boot_order = system.boot_order().expect("boot order present");
    assert_eq!(boot_order.len(), 2);
    assert_eq!(boot_order[0].inner(), "Boot0019: Ubuntu");

    let collection = system.boot_options().await?.expect("boot options link");
    let options = collection.members().await?;
    assert_eq!(options.len(), 2);
    assert!(system.boot_option_matches_boot_order_entry(&options[0], boot_order[0]));
    assert!(!system.boot_option_matches_boot_order_entry(&options[0], boot_order[1]));
    assert!(system.boot_option_matches_boot_order_entry(&options[1], boot_order[1]));

    Ok(())
}

struct TestIds {
    root_id: ODataId,
    systems_id: String,
    system_0_id: String,
}

fn test_ids() -> TestIds {
    let root_id = ODataId::service_root();
    let systems_id = format!("{root_id}/Systems");
    let system_0_id = format!("{systems_id}/System_0");
    TestIds {
        root_id,
        systems_id,
        system_0_id,
    }
}

async fn get_system_0(
    bmc: Arc<Bmc>,
    ids: &TestIds,
    boot_fields: Value,
) -> Result<ComputerSystem<Bmc>, Box<dyn StdError>> {
    let service_root = expect_vera_rubin_service_root(
        bmc.clone(),
        ids,
        json!({
            "Systems": { ODATA_ID: &ids.systems_id }
        }),
    )
    .await?;

    bmc.expect(Expect::expand(
        &ids.systems_id,
        json!({
            ODATA_ID: &ids.systems_id,
            ODATA_TYPE: SYSTEM_COLLECTION_DATA_TYPE,
            "Name": "Computer System Collection",
            "Members": [
                { ODATA_ID: &ids.system_0_id }
            ]
        }),
    ));
    bmc.expect(Expect::get(
        &ids.system_0_id,
        system_0_payload(ids, boot_fields),
    ));

    let systems = service_root.systems().await?.unwrap();
    let mut members = systems.members().await?;
    members.pop().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing System_0").into()
    })
}

async fn expect_vera_rubin_service_root(
    bmc: Arc<Bmc>,
    ids: &TestIds,
    fields: Value,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        json_merge([
            &json!({
                ODATA_ID: &ids.root_id,
                ODATA_TYPE: SERVICE_ROOT_DATA_TYPE,
                "Id": "RootService",
                "Name": "Root Service",
                "Vendor": "NVIDIA",
                "Product": "VR NVL72",
                "RedfishVersion": "1.17.0",
                "ProtocolFeaturesSupported": {
                    "ExpandQuery": {
                        "NoLinks": true
                    }
                },
                "Links": {
                    "Sessions": {
                        ODATA_ID: format!("{}/SessionService/Sessions", ids.root_id),
                    }
                },
            }),
            &fields,
        ]),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}

fn system_0_payload(ids: &TestIds, extra_fields: Value) -> Value {
    json_merge([
        &json!({
            ODATA_ID: &ids.system_0_id,
            ODATA_TYPE: SYSTEM_DATA_TYPE,
            "Id": "System_0",
            "Name": "System_0",
            "Status": {
                "Health": "OK",
                "State": "Enabled"
            },
        }),
        &extra_fields,
    ])
}
