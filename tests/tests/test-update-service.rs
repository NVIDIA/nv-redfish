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

//! Integration tests of Update Service.

#![recursion_limit = "256"]

use nv_redfish::update_service::UpdateService;
use nv_redfish::ServiceRoot;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde_json::json;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::test;

const UPDATE_SERVICE_DATA_TYPE: &str = "#UpdateService.v1_9_0.UpdateService";
const SW_INVENTORIES_DATA_TYPE: &str = "#SoftwareInventoryCollection.SoftwareInventoryCollection";
const SW_INVENTORY_DATA_TYPE: &str = "#SoftwareInventory.v1_4_0.SoftwareInventory";

#[test]
async fn list_dell_fw_inventores() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let update_service = get_update_service(bmc.clone(), &root_id, "Dell").await?;
    let update_service_raw = update_service.raw();
    let update_service_id = update_service_raw.id();
    let fw_inventories_id = format!("{update_service_id}/FirmwareInventory");
    let fw_inventory_id =
        format!("{fw_inventories_id}/Installed-0-2.1.3__Disk.Bay.0:Enclosure.Internal.0-1");
    bmc.expect(Expect::expand(
        &fw_inventories_id,
        json!({
            ODATA_ID: &fw_inventory_id,
            ODATA_TYPE: &SW_INVENTORIES_DATA_TYPE,
            "Name": "Firmware Inventory Collection",
            "Members": [
                {
                    "@odata.id": &fw_inventory_id,
                    "@odata.type": &SW_INVENTORY_DATA_TYPE,
                    "Id": "Installed-0-1.0.0__Disk.Bay.0:Enclosure.Internal.0-1",
                    "Name": "PCIe SSD in Slot 0 in Bay 1",
                    "ReleaseDate": "00:00:00Z",
                    "SoftwareId": "0",
                    "Status": {
                        "Health": "OK",
                        "State": "Enabled"
                    },
                    "Updateable": true,
                    "Version": "1.0.0"
                }
            ]
        }),
    ));
    let inventories = update_service.firmware_inventories().await?.unwrap();
    assert_eq!(inventories.len(), 1);
    assert!(inventories[0].raw().release_date.is_none());
    Ok(())
}

async fn get_update_service(
    bmc: Arc<Bmc>,
    root_id: &ODataId,
    vendor: &str,
) -> Result<UpdateService<Bmc>, Box<dyn StdError>> {
    let update_service_id = format!("{root_id}/UpdateService");
    let data_type = "#ServiceRoot.v1_13_0.ServiceRoot";
    bmc.expect(Expect::get(
        &root_id,
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
            "Id": "RootService",
            "Name": "RootService",
            "ProtocolFeaturesSupported": {
                "ExpandQuery": {
                    "NoLinks": true
                }
            },
            "UpdateService": {
                ODATA_ID: &update_service_id,
            },
            "Vendor": vendor,
            "Links": {},
        }),
    ));
    let service_root = ServiceRoot::new(bmc.clone()).await?;

    let fw_inventory_id = format!("{update_service_id}/FirmwareInventory");
    bmc.expect(Expect::get(
        &update_service_id,
        json!({
            ODATA_ID: &update_service_id,
            ODATA_TYPE: &UPDATE_SERVICE_DATA_TYPE,
            "Id": "UpdateService",
            "Name": "UpdateService",
            "FirmwareInventory": {
                ODATA_ID: &fw_inventory_id,
            },
        }),
    ));
    Ok(service_root.update_service().await?.unwrap())
}
