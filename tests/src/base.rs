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

pub mod redfish {
    include!(concat!(env!("OUT_DIR"), "/base_tests.rs"));
}

use crate::Expect;
use crate::ODATA_ID;
use crate::ODATA_TYPE;
use nv_redfish_core::Bmc as NvRedfishBmc;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataId;
use redfish::service_root::ServiceRoot;
use serde_json::json;
use std::sync::Arc;

pub async fn get_service_root<Bmc>(bmc: &Bmc) -> Result<Arc<ServiceRoot>, Bmc::Error>
where
    Bmc: NvRedfishBmc,
{
    NavProperty::<ServiceRoot>::new_reference(ODataId::service_root())
        .get(bmc)
        .await
}

pub fn nav_service_root() -> NavProperty<ServiceRoot> {
    NavProperty::<ServiceRoot>::new_reference(ODataId::service_root())
}

pub fn expect_root() -> Expect {
    let root_id = ODataId::service_root();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    Expect::get(
        root_id.clone(),
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
        }),
    )
}

pub fn expect_root_srv(service_name: &str, service_id: &str) -> Expect {
    let root_id = ODataId::service_root();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    Expect::get(
        root_id.clone(),
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
            service_name: { ODATA_ID: &service_id },
        }),
    )
}
