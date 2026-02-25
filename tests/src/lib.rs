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

//! This is tests support lib.

/// Schema compiled for base tests.
pub mod base;
/// Errors used in tests.
pub mod error;
/// Expectations in tests.
pub mod json_merge;

#[doc(inline)]
pub use error::Error;
#[doc(inline)]
pub use json_merge::json_merge;

/// Used in tests for `@odata.id` fields.
pub const ODATA_ID: &str = "@odata.id";
/// Used in tests for `@odata.type` fields.
pub const ODATA_TYPE: &str = "@odata.type";

use error::TestError;
use nv_redfish_bmc_mock::Bmc as MockBmc;
use nv_redfish_bmc_mock::Expect as MockExpect;
use nv_redfish_core::ODataId;
use serde_json::json;
use serde_json::Value;

pub type Bmc = MockBmc<TestError>;
pub type Expect = MockExpect<TestError>;

/// Build a ServiceRoot payload for AMI Viking (`Vendor=AMI`, `RedfishVersion=1.11.0`)
/// merged with the provided `fields`.
pub fn ami_viking_service_root(root_id: &ODataId, fields: Value) -> Value {
    let base = json!({
        ODATA_ID: root_id,
        ODATA_TYPE: "#ServiceRoot.v1_13_0.ServiceRoot",
        "Id": "RootService",
        "Name": "RootService",
        "ProtocolFeaturesSupported": {
            "ExpandQuery": {
                "NoLinks": true
            }
        },
        "Vendor": "AMI",
        "RedfishVersion": "1.11.0",
        "Links": {},
    });
    json_merge([&base, &fields])
}
