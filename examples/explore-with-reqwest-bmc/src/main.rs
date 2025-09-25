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

#![recursion_limit = "256"]

use nv_redfish::Expandable;
use nv_redfish::ODataId;
use nv_redfish::bmc::BmcCredentials;
use nv_redfish::http::BmcReqwestError;
use nv_redfish::http::ExpandQuery;
use nv_redfish::http::HttpBmc;
use nv_redfish::http::ReqwestClient;
use nv_redfish::http::ReqwestClientParams;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), BmcReqwestError> {
    let client = ReqwestClient::with_params(ReqwestClientParams::new().accept_invalid_certs(true))
        .map_err(BmcReqwestError::ReqwestError)?;

    let creds = BmcCredentials::new("username".into(), "password".into());
    let bmc = HttpBmc::new(client, Url::parse("https://192.168.2.2").unwrap(), creds);

    let service_root =
        nv_redfish::NavProperty::<redfish_std::redfish::service_root::ServiceRoot>::new_reference(
            ODataId::service_root(),
        )
        .get(&bmc)
        .await?;

    let chassis_members = &service_root
        .chassis
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;

    let chassis = chassis_members.iter().next().unwrap().get(&bmc).await?;

    let all_devices = &chassis
        .pc_ie_devices
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;

    for device in all_devices {
        if let Some(nav_prop) = &device.get(&bmc).await?.pc_ie_functions {
            let function_handles = nav_prop.get(&bmc).await?;
            for function_handle in &function_handles.members {
                let _function = function_handle.get(&bmc).await?;
            }

            println!("{function_handles:?}"); // unpolulated members
            let function_handles = function_handles
                .expand(&bmc, ExpandQuery::default())
                .await?;
            println!("{function_handles:?}"); // memberrs populated
        }
    }

    let system = &service_root
        .systems
        .as_ref()
        .expect("no systems")
        .get(&bmc)
        .await?
        .members
        .first()
        .expect("at least one system")
        .get(&bmc)
        .await?;

    println!("{system:?}");

    let ac = &service_root
        .account_service
        .as_ref()
        .expect("no account service")
        .get(&bmc)
        .await?;
    println!("{ac:?}");

    // Should use cache here
    let _ = &service_root
        .account_service
        .as_ref()
        .expect("no account service")
        .get(&bmc)
        .await?;

    system
        .bios
        .as_ref()
        .expect("no bios")
        .get(&bmc)
        .await?
        .actions
        .as_ref()
        .expect("no actions")
        .change_password
        .as_ref()
        .expect("no reset action")
        .run(
            &bmc,
            &redfish_std::redfish::bios::BiosChangePasswordAction {
                password_name: Some("admin".into()),
                old_password: Some("admin1".into()),
                new_password: Some("admin2".into()),
            },
        )
        .await?;

    Ok(())
}
