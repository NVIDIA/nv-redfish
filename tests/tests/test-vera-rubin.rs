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
//! Integration tests for VeraRubin (VR NVL72) host BMC Redfish payloads.
//!
//! Payload shapes are taken from a Redfish mockup capture of BMC 10.73.114.74
//! (`Vendor=NVIDIA`, `Product=VR NVL72`, klamath-dev1-dh1).

use nv_redfish::chassis::Chassis;
use nv_redfish::computer_system::ComputerSystem;
use nv_redfish::hardware_id::Model;
use nv_redfish::hardware_id::PartNumber;
use nv_redfish::hardware_id::SerialNumber;
use nv_redfish::ServiceRoot;
use nv_redfish_core::EdmGuid;
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
const CHASSIS_COLLECTION_DATA_TYPE: &str = "#ChassisCollection.ChassisCollection";
const CHASSIS_DATA_TYPE: &str = "#Chassis.v1_22_0.Chassis";
const ASSEMBLY_DATA_TYPE: &str = "#Assembly.v1_3_0.Assembly";
const SYSTEM_COLLECTION_DATA_TYPE: &str = "#ComputerSystemCollection.ComputerSystemCollection";
const SYSTEM_DATA_TYPE: &str = "#ComputerSystem.v1_22_0.ComputerSystem";

const BMC_0_SERIAL: &str = "1331026110486";
const SYSTEM_0_SERIAL: &str = "1331226010198";
const SYSTEM_0_UUID: &str = "3f19f49b-c376-a7b1-bd5c-14fe79a827aa";
const BLUEFIELD_0_UUID: &str = "d8453486-733e-f111-8000-f4204d495204";

#[test]
async fn vera_rubin_bmc_0_chassis_parses() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let chassis = get_bmc_0_chassis(bmc.clone(), &ids).await?;

    let hw = chassis.hardware_id();
    assert_eq!(hw.model, Some(Model::new("VR NVL72")));
    assert_eq!(hw.serial_number, Some(SerialNumber::new(BMC_0_SERIAL)));
    assert_eq!(
        hw.part_number,
        Some(PartNumber::new("699-23809-0610-TS4"))
    );
    assert_eq!(chassis.raw().base.id, "BMC_0");

    Ok(())
}

#[test]
async fn vera_rubin_bmc_0_assembly_members_without_odata_type_parse(
) -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let chassis = get_bmc_0_chassis(bmc.clone(), &ids).await?;

    bmc.expect(Expect::expand(
        &ids.bmc_0_assembly_id,
        bmc_0_assembly_payload(&ids),
    ));
    let assembly = chassis.assembly().await?.unwrap();
    let members = assembly.assemblies().await?;
    assert_eq!(members.len(), 2);

    let board = &members[0];
    let hw = board.hardware_id();
    assert_eq!(hw.model, Some(Model::new("P3809")));
    assert_eq!(
        hw.part_number,
        Some(PartNumber::new("699-23809-0610-TS4"))
    );
    assert_eq!(hw.serial_number, Some(SerialNumber::new(BMC_0_SERIAL)));

    let product = &members[1];
    assert_eq!(
        product.hardware_id().model,
        Some(Model::new("P3809-BMC"))
    );

    Ok(())
}

#[test]
async fn vera_rubin_systems_collection_members_parse() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let service_root = expect_vera_rubin_service_root(
        bmc.clone(),
        &ids,
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
                { ODATA_ID: &ids.hgx_baseboard_id },
                { ODATA_ID: &ids.system_0_id },
            ],
            "Members@odata.count": 2
        }),
    ));
    bmc.expect(Expect::get(
        &ids.hgx_baseboard_id,
        system_payload(&ids.hgx_baseboard_id, "HGX_Baseboard_0", json!({})),
    ));
    bmc.expect(Expect::get(
        &ids.system_0_id,
        system_0_payload(&ids, json!({})),
    ));

    let systems = service_root.systems().await?.unwrap();
    let members = systems.members().await?;
    assert_eq!(members.len(), 2);

    Ok(())
}

#[test]
async fn vera_rubin_system_0_parses() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let system = get_system_0(bmc.clone(), &ids).await?;

    let hw = system.hardware_id();
    assert_eq!(hw.model, Some(Model::new("VR NVL72")));
    assert_eq!(hw.serial_number, Some(SerialNumber::new(SYSTEM_0_SERIAL)));
    assert_eq!(
        hw.part_number,
        Some(PartNumber::new("699-24107-0210-TS3"))
    );
    assert_eq!(
        system.raw().uuid,
        Some(Some(EdmGuid::parse_str(SYSTEM_0_UUID).unwrap()))
    );
    assert!(system.raw().bios.is_some());

    Ok(())
}

#[test]
async fn vera_rubin_composite_boot_order_matches_boot_options() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let boot_options_id = format!("{}/BootOptions", ids.system_0_id);
    let boot0019_id = format!("{boot_options_id}/Boot0019");
    let boot0010_id = format!("{boot_options_id}/Boot0010");

    let system = get_system_0_with_boot(
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

#[test]
async fn vera_rubin_bluefield_0_chassis_uuid_parses() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = test_ids();
    let service_root = expect_vera_rubin_service_root(
        bmc.clone(),
        &ids,
        json!({
            "Chassis": { ODATA_ID: &ids.chassis_collection_id }
        }),
    )
    .await?;

    bmc.expect(Expect::expand(
        &ids.chassis_collection_id,
        json!({
            ODATA_ID: &ids.chassis_collection_id,
            ODATA_TYPE: CHASSIS_COLLECTION_DATA_TYPE,
            "Name": "Chassis Collection",
            "Members": [
                { ODATA_ID: &ids.bluefield_0_id }
            ]
        }),
    ));
    bmc.expect(Expect::get(
        &ids.bluefield_0_id,
        bluefield_0_chassis_payload(&ids),
    ));

    let collection = service_root.chassis().await?.unwrap();
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);
    assert_eq!(
        members[0].raw().uuid,
        Some(Some(EdmGuid::parse_str(BLUEFIELD_0_UUID).unwrap()))
    );

    Ok(())
}

async fn get_bmc_0_chassis(bmc: Arc<Bmc>, ids: &TestIds) -> Result<Chassis<Bmc>, Box<dyn StdError>> {
    let service_root = expect_vera_rubin_service_root(
        bmc.clone(),
        ids,
        json!({
            "Chassis": { ODATA_ID: &ids.chassis_collection_id }
        }),
    )
    .await?;

    bmc.expect(Expect::expand(
        &ids.chassis_collection_id,
        json!({
            ODATA_ID: &ids.chassis_collection_id,
            ODATA_TYPE: CHASSIS_COLLECTION_DATA_TYPE,
            "Name": "Chassis Collection",
            "Members": [
                { ODATA_ID: &ids.bmc_0_id }
            ]
        }),
    ));
    bmc.expect(Expect::get(
        &ids.bmc_0_id,
        bmc_0_chassis_payload(ids),
    ));

    let collection = service_root.chassis().await?.unwrap();
    let mut members = collection.members().await?;
    members.pop().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing BMC_0 chassis").into()
    })
}

async fn get_system_0(
    bmc: Arc<Bmc>,
    ids: &TestIds,
) -> Result<ComputerSystem<Bmc>, Box<dyn StdError>> {
    get_system_0_with_boot(bmc, ids, json!({})).await
}

async fn get_system_0_with_boot(
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
                "UUID": "bcbdca99-203d-4c5c-8939-276f37d8aef4",
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

struct TestIds {
    root_id: ODataId,
    chassis_collection_id: String,
    bmc_0_id: String,
    bmc_0_assembly_id: String,
    bluefield_0_id: String,
    systems_id: String,
    hgx_baseboard_id: String,
    system_0_id: String,
}

fn test_ids() -> TestIds {
    let root_id = ODataId::service_root();
    let chassis_collection_id = format!("{root_id}/Chassis");
    let bmc_0_id = format!("{chassis_collection_id}/BMC_0");
    let bmc_0_assembly_id = format!("{bmc_0_id}/Assembly");
    let bluefield_0_id = format!("{chassis_collection_id}/BlueField_0");
    let systems_id = format!("{root_id}/Systems");
    let hgx_baseboard_id = format!("{systems_id}/HGX_Baseboard_0");
    let system_0_id = format!("{systems_id}/System_0");
    TestIds {
        root_id,
        chassis_collection_id,
        bmc_0_id,
        bmc_0_assembly_id,
        bluefield_0_id,
        systems_id,
        hgx_baseboard_id,
        system_0_id,
    }
}

fn bmc_0_chassis_payload(ids: &TestIds) -> Value {
    json!({
        ODATA_ID: &ids.bmc_0_id,
        ODATA_TYPE: CHASSIS_DATA_TYPE,
        "Id": "BMC_0",
        "Name": "BMC_0",
        "ChassisType": "Module",
        "Manufacturer": "NVIDIA",
        "Model": "VR NVL72",
        "PartNumber": "699-23809-0610-TS4",
        "SerialNumber": BMC_0_SERIAL,
        "Assembly": {
            ODATA_ID: &ids.bmc_0_assembly_id
        },
        "Status": {
            "Health": "OK",
            "HealthRollup": "OK",
            "State": "Enabled"
        }
    })
}

fn bmc_0_assembly_payload(ids: &TestIds) -> Value {
    json!({
        ODATA_ID: &ids.bmc_0_assembly_id,
        ODATA_TYPE: ASSEMBLY_DATA_TYPE,
        "Id": "Assembly",
        "Name": "Assembly data for BMC_0",
        "Assemblies": [
            {
                ODATA_ID: format!("{}/#/Assemblies/0", ids.bmc_0_assembly_id),
                "Location": {
                    "PartLocation": {
                        "LocationType": "Embedded"
                    }
                },
                "MemberId": "0",
                "Model": "P3809",
                "Name": "BMC Board FRU Assembly0",
                "PartNumber": "699-23809-0610-TS4",
                "ProductionDate": "2026-03-05T19:20:00Z",
                "SerialNumber": BMC_0_SERIAL,
                "Vendor": "NVIDIA"
            },
            {
                ODATA_ID: format!("{}/#/Assemblies/1", ids.bmc_0_assembly_id),
                "Location": {
                    "PartLocation": {
                        "LocationType": "Embedded"
                    }
                },
                "MemberId": "1",
                "Model": "P3809-BMC",
                "Name": "BMC Product FRU Assembly1",
                "PartNumber": "699-23809-0610-TS4",
                "SerialNumber": BMC_0_SERIAL,
                "Vendor": "NVIDIA",
                "Version": "C01"
            }
        ]
    })
}

fn system_payload(id: &str, name: &str, fields: Value) -> Value {
    let base = json!({
        ODATA_ID: id,
        ODATA_TYPE: SYSTEM_DATA_TYPE,
        "Id": name,
        "Name": name,
        "Status": {
            "Health": "OK",
            "State": "Enabled"
        }
    });
    json_merge([&base, &fields])
}

fn system_0_payload(ids: &TestIds, extra_fields: Value) -> Value {
    system_payload(
        &ids.system_0_id,
        "System_0",
        json_merge([
            &json!({
                "Manufacturer": "NVIDIA",
                "Model": "VR NVL72",
                "PartNumber": "699-24107-0210-TS3",
                "SerialNumber": SYSTEM_0_SERIAL,
                "SystemType": "Physical",
                "UUID": SYSTEM_0_UUID,
                "Bios": {
                    ODATA_ID: format!("{}/Bios", ids.system_0_id)
                },
                "Links": {
                    "Chassis": [
                        { ODATA_ID: &ids.bmc_0_id }
                    ]
                }
            }),
            &extra_fields,
        ]),
    )
}

fn bluefield_0_chassis_payload(ids: &TestIds) -> Value {
    json!({
        ODATA_ID: &ids.bluefield_0_id,
        ODATA_TYPE: CHASSIS_DATA_TYPE,
        "Id": "BlueField_0",
        "Name": "BlueField_0",
        "ChassisType": "Component",
        "Manufacturer": "NVIDIA",
        "Model": "NVIDIA BlueField-4 B4240V 800G Liquid Cooled DPU, Dual-port 400GbE / NDR, QSFP112, PCIe Gen6 x16, 64 Arm cores, 128GB LPDDR5x, integrated BMC, Crypto Enabled, Secure Boot Enabled",
        "PartNumber": "900-9D4A4-00CB-TS4",
        "SerialNumber": "MT2617601WT5",
        "UUID": BLUEFIELD_0_UUID,
        "Status": {
            "Health": "OK",
            "HealthRollup": "OK",
            "State": "Enabled"
        }
    })
}
