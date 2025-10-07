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

#[cfg(feature = "reqwest")]
#[allow(dead_code)]
pub mod test_utils {
    use nv_redfish_core::{
        EntityTypeRef, Expandable, ODataETag, ODataId,
        action::Action,
        bmc::BmcCredentials,
        http::{HttpBmc, ReqwestClient},
    };
    use serde::{Deserialize, Serialize};
    use url::Url;
    use wiremock::MockServer;

    /// Test resource struct used across integration tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct TestResource {
        #[serde(rename = "@odata.id")]
        pub id: ODataId,
        #[serde(rename = "@odata.etag")]
        pub etag: Option<ODataETag>,
        pub name: String,
        pub value: i32,
    }

    impl EntityTypeRef for TestResource {
        fn id(&self) -> &ODataId {
            &self.id
        }

        fn etag(&self) -> Option<&ODataETag> {
            self.etag.as_ref()
        }
    }

    impl Expandable for TestResource {}

    /// Fake resources for test HTTP requests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct CreateRequest {
        pub name: String,
        pub value: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct UpdateRequest {
        pub name: Option<String>,
        pub value: Option<i32>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ActionRequest {
        pub parameter: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ActionResponse {
        pub result: String,
        pub success: bool,
    }

    pub fn create_odata_id(s: &str) -> ODataId {
        ODataId::from(s.to_string())
    }

    pub fn create_odata_etag(s: &str) -> ODataETag {
        ODataETag::from(s.to_string())
    }

    pub fn create_test_action(target: &str) -> Action<ActionRequest, ActionResponse> {
        let json = format!(r#"{{"target": "{}"}}"#, target);
        serde_json::from_str(&json).unwrap()
    }

    pub fn create_test_resource(
        path: &str,
        etag: Option<&str>,
        name: &str,
        value: i32,
    ) -> TestResource {
        TestResource {
            id: create_odata_id(path),
            etag: etag.map(create_odata_etag),
            name: name.to_string(),
            value,
        }
    }

    pub fn create_test_credentials() -> BmcCredentials {
        BmcCredentials::new("root".to_string(), "password".to_string())
    }

    pub fn create_test_bmc(mock_server: &MockServer) -> HttpBmc<ReqwestClient> {
        let client = ReqwestClient::new().unwrap();
        let credentials = create_test_credentials();
        HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials)
    }

    pub fn create_test_bmc_with_credentials(
        mock_server: &MockServer,
        credentials: BmcCredentials,
    ) -> HttpBmc<ReqwestClient> {
        let client = ReqwestClient::new().unwrap();
        HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials)
    }

    pub mod names {
        pub const TEST_CHASSIS: &str = "Test Chassis";
        pub const TEST_SYSTEM: &str = "Test System";
        pub const TEST_MANAGER: &str = "Test Manager";
    }

    pub mod paths {
        pub const CHASSIS_1: &str = "/redfish/v1/Chassis/1";
        pub const MANAGERS_1: &str = "/redfish/v1/Managers/1";
        pub const SYSTEMS_1: &str = "/redfish/v1/Systems/1";
        pub const NONEXISTENT: &str = "/redfish/v1/nonexistent";
    }
}
