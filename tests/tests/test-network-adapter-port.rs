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

//! Integration tests for network adapter port discovery.

use nv_redfish::chassis::NetworkAdapter;
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

#[test]
async fn adapter_ports_report_standard_and_lenovo_mac_addresses() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = Ids::new();
    let adapter = get_adapter(bmc.clone(), &ids, true).await?;
    bmc.expect(Expect::get(
        &ids.ports_id,
        collection_payload(&ids.ports_id, &ids.port_ids),
    ));
    let Some(collection) = adapter.ports().await? else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing port collection",
        )
        .into());
    };

    let port_fields = [
        json!({
            "Ethernet": {
                "AssociatedMACAddresses": ["10:00:00:00:00:01", "10:00:00:00:00:02"]
            }
        }),
        json!({
            "Oem": {
                "Lenovo": {
                    ODATA_TYPE: "#LenovoPort.v1_0_0.Port",
                    "PhysicalPortMacAddress": "20:00:00:00:00:01"
                }
            }
        }),
        json!({
            "Ethernet": {
                "AssociatedMACAddresses": ["30:00:00:00:00:01"]
            },
            "Oem": {
                "Lenovo": {
                    ODATA_TYPE: "#LenovoPort.v1_0_0.Port",
                    "PhysicalPortMacAddress": "30:00:00:00:00:02"
                }
            }
        }),
        json!({
            "Ethernet": {
                "AssociatedMACAddresses": []
            },
            "Oem": {
                "Lenovo": {
                    ODATA_TYPE: "#LenovoPort.v1_0_0.Port",
                    "PhysicalPortMacAddress": "40:00:00:00:00:01"
                }
            }
        }),
        json!({}),
    ];
    for (port_id, fields) in ids.port_ids.iter().zip(port_fields) {
        bmc.expect(Expect::get(port_id, port_payload(port_id, fields)));
    }

    let mut addresses = Vec::new();
    for port in collection.members().await? {
        addresses.push(
            port.associated_mac_addresses()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        );
    }

    assert_eq!(
        addresses,
        [
            vec!["10:00:00:00:00:01", "10:00:00:00:00:02"],
            vec!["20:00:00:00:00:01"],
            vec!["30:00:00:00:00:01"],
            vec!["40:00:00:00:00:01"],
            vec![],
        ]
    );
    Ok(())
}

#[test]
async fn adapter_without_ports_link_returns_none() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = Ids::new();
    let adapter = get_adapter(bmc, &ids, false).await?;

    assert!(adapter.ports().await?.is_none());
    Ok(())
}

async fn get_adapter(
    bmc: Arc<Bmc>,
    ids: &Ids,
    with_ports: bool,
) -> Result<NetworkAdapter<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(&ids.root_id, service_root_payload(ids)));
    let root = ServiceRoot::new(bmc.clone()).await?;

    bmc.expect(Expect::get(
        &ids.chassis_collection_id,
        collection_payload(
            &ids.chassis_collection_id,
            std::slice::from_ref(&ids.chassis_id),
        ),
    ));
    let Some(chassis_collection) = root.chassis().await? else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing chassis collection",
        )
        .into());
    };

    bmc.expect(Expect::get(&ids.chassis_id, chassis_payload(ids)));
    let mut chassis = chassis_collection.members().await?;
    let Some(chassis) = chassis.pop() else {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing chassis").into());
    };

    bmc.expect(Expect::get(
        &ids.adapters_id,
        collection_payload(&ids.adapters_id, std::slice::from_ref(&ids.adapter_id)),
    ));
    bmc.expect(Expect::get(
        &ids.adapter_id,
        adapter_payload(ids, with_ports),
    ));
    let mut adapters = chassis.network_adapters().await?.unwrap_or_default();
    adapters.pop().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing network adapter").into()
    })
}

fn service_root_payload(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.root_id,
        ODATA_TYPE: "#ServiceRoot.v1_13_0.ServiceRoot",
        "Id": "RootService",
        "Name": "RootService",
        "ProtocolFeaturesSupported": {
            "ExpandQuery": {
                "NoLinks": false
            }
        },
        "Chassis": { ODATA_ID: &ids.chassis_collection_id },
        "Links": {
            "Sessions": {
                ODATA_ID: format!("{}/SessionService/Sessions", ids.root_id),
            }
        },
    })
}

fn collection_payload(collection_id: &str, member_ids: &[String]) -> Value {
    json!({
        ODATA_ID: collection_id,
        ODATA_TYPE: "#Resource.ResourceCollection",
        "Name": "Collection",
        "Members": member_ids
            .iter()
            .map(|id| json!({ ODATA_ID: id }))
            .collect::<Vec<_>>(),
    })
}

fn chassis_payload(ids: &Ids) -> Value {
    json!({
        ODATA_ID: &ids.chassis_id,
        ODATA_TYPE: "#Chassis.v1_23_0.Chassis",
        "Id": "1",
        "Name": "Chassis",
        "ChassisType": "RackMount",
        "NetworkAdapters": { ODATA_ID: &ids.adapters_id },
    })
}

fn adapter_payload(ids: &Ids, with_ports: bool) -> Value {
    let mut payload = json!({
        ODATA_ID: &ids.adapter_id,
        ODATA_TYPE: "#NetworkAdapter.v1_10_0.NetworkAdapter",
        "Id": "1",
        "Name": "Network Adapter",
    });
    if with_ports {
        payload["Ports"] = json!({ ODATA_ID: &ids.ports_id });
    }
    payload
}

fn port_payload(port_id: &str, fields: Value) -> Value {
    let base = json!({
        ODATA_ID: port_id,
        ODATA_TYPE: "#Port.v1_10_0.Port",
        "Id": port_id.rsplit('/').next().unwrap_or_default(),
        "Name": "Port",
    });
    json_merge([&base, &fields])
}

struct Ids {
    root_id: ODataId,
    chassis_collection_id: String,
    chassis_id: String,
    adapters_id: String,
    adapter_id: String,
    ports_id: String,
    port_ids: Vec<String>,
}

impl Ids {
    fn new() -> Self {
        let root_id = ODataId::service_root();
        let chassis_collection_id = format!("{root_id}/Chassis");
        let chassis_id = format!("{chassis_collection_id}/1");
        let adapters_id = format!("{chassis_id}/NetworkAdapters");
        let adapter_id = format!("{adapters_id}/1");
        let ports_id = format!("{adapter_id}/Ports");
        let port_ids = (1..=5).map(|id| format!("{ports_id}/{id}")).collect();
        Self {
            root_id,
            chassis_collection_id,
            chassis_id,
            adapters_id,
            adapter_id,
            ports_id,
            port_ids,
        }
    }
}
