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
//! Integration tests for Computer System resources.

#![recursion_limit = "256"]

use nv_redfish::computer_system::SystemCollection;
use nv_redfish::ServiceRoot;
use nv_redfish_core::ODataId;
use nv_redfish_tests::ami_viking_service_root;
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
const SYSTEM_DATA_TYPE: &str = "#ComputerSystem.v1_20_0.ComputerSystem";

#[test]
async fn dell_wrong_last_reset_time_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = computer_system_ids();
    let computer_system = computer_system(
        &ids,
        json!({ "LastResetTime": "0000-00-00T00:00:00+00:00" }),
    );
    let systems = get_systems(bmc.clone(), &ids, "Dell", vec![computer_system]).await?;

    let members = systems.members().await?;
    assert_eq!(members.len(), 1);
    let system = &members[0];
    assert!(system.raw().last_reset_time.is_none());

    Ok(())
}

#[test]
async fn ami_viking_missing_root_systems_nav_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = computer_system_ids();
    let computer_system = computer_system(&ids, json!({}));
    let service_root = expect_viking_service_root_without_systems(bmc.clone(), &ids).await?;
    bmc.expect(Expect::get(
        &ids.systems_id,
        json!({
            ODATA_ID: &ids.systems_id,
            ODATA_TYPE: &SYSTEM_COLLECTION_DATA_TYPE,
            "Id": resource_name(&ids.systems_id),
            "Name": "Computer System Collection",
            "Members": [computer_system]
        }),
    ));

    let systems = service_root.systems().await?.unwrap();
    let members = systems.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

async fn get_systems(
    bmc: Arc<Bmc>,
    ids: &ComputerSystemIds,
    vendor: &str,
    members: Vec<Value>,
) -> Result<SystemCollection<Bmc>, Box<dyn StdError>> {
    let service_root = expect_service_root(bmc.clone(), ids, vendor).await?;
    let systems_name = resource_name(&ids.systems_id);
    bmc.expect(Expect::expand(
        &ids.systems_id,
        json!({
            ODATA_ID: &ids.systems_id,
            ODATA_TYPE: &SYSTEM_COLLECTION_DATA_TYPE,
            "Id": systems_name,
            "Name": "Computer System Collection",
            "Members": members
        }),
    ));

    service_root
        .systems()
        .await
        .map(Option::unwrap)
        .map_err(Into::into)
}

async fn expect_service_root(
    bmc: Arc<Bmc>,
    ids: &ComputerSystemIds,
    vendor: &str,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        json!({
            ODATA_ID: &ids.root_id,
            ODATA_TYPE: &SERVICE_ROOT_DATA_TYPE,
            "Id": "RootService",
            "Name": "RootService",
            "ProtocolFeaturesSupported": {
                "ExpandQuery": {
                    "NoLinks": true
                }
            },
            "Systems": { ODATA_ID: &ids.systems_id },
            "Vendor": vendor,
            "Links": {},
        }),
    ));

    ServiceRoot::new(bmc).await.map_err(Into::into)
}

async fn expect_viking_service_root_without_systems(
    bmc: Arc<Bmc>,
    ids: &ComputerSystemIds,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        ami_viking_service_root(&ids.root_id, json!({})),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}

struct ComputerSystemIds {
    root_id: ODataId,
    systems_id: String,
    system_id: String,
}

fn computer_system_ids() -> ComputerSystemIds {
    let root_id = ODataId::service_root();
    let systems_id = format!("{root_id}/Systems");
    let system_id = format!("{systems_id}/System-1");
    ComputerSystemIds {
        root_id,
        systems_id,
        system_id,
    }
}

fn resource_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

fn computer_system(ids: &ComputerSystemIds, fields: Value) -> Value {
    let override_id = fields
        .as_object()
        .and_then(|obj| obj.get(ODATA_ID))
        .and_then(Value::as_str);
    let system_id = override_id.unwrap_or_else(|| ids.system_id.as_str());
    let name = resource_name(system_id);
    let base = json!({
        ODATA_ID: system_id,
        ODATA_TYPE: &SYSTEM_DATA_TYPE,
        "Id": name,
        "Name": name,
        "Status": {
            "Health": "OK",
            "State": "Enabled"
        }
    });
    json_merge([&base, &fields])
}
