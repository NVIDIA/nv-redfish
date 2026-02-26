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
//! Integration tests for Supermicro Manager OEM support.

#![recursion_limit = "256"]

use nv_redfish::manager::Manager;
use nv_redfish::oem::supermicro::Privilege;
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
const SUPERMICRO_MANAGER_DATA_TYPE: &str = "#SmcManagerExtensions.v1_0_0.Manager";
const KCS_INTERFACE_DATA_TYPE: &str = "#KCSInterface.v1_1_0.KCSInterface";
const SYS_LOCKDOWN_DATA_TYPE: &str = "#SysLockdown.v1_0_0.SysLockdown";

#[test]
async fn supermicro_kcs_and_sys_lockdown_supported() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(
        bmc.clone(),
        &ids,
        manager_payload(
            &ids,
            Some(ids.kcs_interface_ref()),
            Some(ids.sys_lockdown_ref()),
        ),
    )
    .await?;

    let supermicro = manager.oem_supermicro()?.unwrap();
    bmc.expect(Expect::get(
        &ids.kcs_interface_id,
        kcs_interface_payload(&ids),
    ));
    let kcs = supermicro.kcs_interface().await?.unwrap();
    assert_eq!(kcs.privilege(), Some(Privilege::Administrator));

    bmc.expect(Expect::get(
        &ids.sys_lockdown_id,
        sys_lockdown_payload(&ids),
    ));
    let lockdown = supermicro.sys_lockdown().await?.unwrap();
    assert_eq!(lockdown.sys_lockdown_enabled(), Some(false));

    Ok(())
}

#[test]
async fn supermicro_manager_without_kcs_still_supports_sys_lockdown(
) -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(
        bmc.clone(),
        &ids,
        manager_payload(&ids, None, Some(ids.sys_lockdown_ref())),
    )
    .await?;

    bmc.expect(Expect::get(
        &ids.sys_lockdown_id,
        sys_lockdown_payload(&ids),
    ));

    let supermicro = manager.oem_supermicro()?.unwrap();
    assert!(supermicro.kcs_interface().await?.is_none());

    let lockdown = supermicro.sys_lockdown().await?.unwrap();
    assert_eq!(lockdown.sys_lockdown_enabled(), Some(false));

    Ok(())
}

#[test]
async fn manager_without_supermicro_oem_returns_none() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let manager = get_manager(bmc.clone(), &ids, manager_payload_without_supermicro(&ids)).await?;

    assert!(manager.oem_supermicro()?.is_none());

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
    kcs_interface_id: String,
    sys_lockdown_id: String,
}

impl Ids {
    fn kcs_interface_ref(&self) -> Value {
        json!({ ODATA_ID: &self.kcs_interface_id })
    }

    fn sys_lockdown_ref(&self) -> Value {
        json!({ ODATA_ID: &self.sys_lockdown_id })
    }
}

fn ids() -> Ids {
    let root_id = ODataId::service_root();
    let managers_id = format!("{root_id}/Managers");
    let manager_id = format!("{managers_id}/1");
    let kcs_interface_id = format!("{manager_id}/Oem/Supermicro/KCSInterface");
    let sys_lockdown_id = format!("{manager_id}/Oem/Supermicro/SysLockdown");
    Ids {
        root_id,
        managers_id,
        manager_id,
        kcs_interface_id,
        sys_lockdown_id,
    }
}

fn manager_payload(ids: &Ids, kcs_interface: Option<Value>, sys_lockdown: Option<Value>) -> Value {
    let base = json!({
        ODATA_ID: &ids.manager_id,
        ODATA_TYPE: MANAGER_DATA_TYPE,
        "Id": "1",
        "Name": "Manager",
        "ManagerType": "BMC",
        "Status": { "State": "Enabled" },
    });

    let mut supermicro = json!({
        ODATA_TYPE: SUPERMICRO_MANAGER_DATA_TYPE,
    });
    if let Some(v) = kcs_interface {
        supermicro["KCSInterface"] = v;
    }
    if let Some(v) = sys_lockdown {
        supermicro["SysLockdown"] = v;
    }

    let oem = json!({
        "Oem": {
            "Supermicro": supermicro
        }
    });
    json_merge([&base, &oem])
}

fn manager_payload_without_supermicro(ids: &Ids) -> Value {
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

fn kcs_interface_payload(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.kcs_interface_id,
        ODATA_TYPE: KCS_INTERFACE_DATA_TYPE,
        "Id": "KCSInterface",
        "Name": "KCS Interface",
        "Privilege": "Administrator",
        "@odata.etag": "\"7f21b53f195494a7c2dad2008917b1d7\""
    })
}

fn sys_lockdown_payload(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.sys_lockdown_id,
        ODATA_TYPE: SYS_LOCKDOWN_DATA_TYPE,
        "Id": "SysLockdown",
        "Name": "SysLockdown",
        "SysLockdownEnabled": false,
        "@odata.etag": "\"30b691549156f2528aac46ed839cf7f6\""
    })
}
