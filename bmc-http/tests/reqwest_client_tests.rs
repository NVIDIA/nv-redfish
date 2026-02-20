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

mod common;

#[cfg(feature = "reqwest")]
mod reqwest_client_tests {
    use nv_redfish_bmc_http::reqwest::BmcError;
    use nv_redfish_core::{
        query::{ExpandQuery, FilterQuery},
        Bmc,
    };
    use wiremock::{
        matchers::{body_json, header, method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::common::test_utils::*;

    #[tokio::test]
    async fn test_get_request_success() {
        let mock_server = MockServer::start().await;
        let resource_path = paths::SYSTEMS_1;

        let test_resource =
            create_test_resource(resource_path, Some("123"), names::TEST_SYSTEM, 42);

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(200).set_body_json(&test_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.get::<TestResource>(&resource_id).await;

        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.name, names::TEST_SYSTEM);
        assert_eq!(retrieved.value, 42);
    }

    #[tokio::test]
    async fn test_get_request_with_expand() {
        let mock_server = MockServer::start().await;
        let resource_path = paths::SYSTEMS_1;

        let test_resource =
            create_test_resource(resource_path, Some("456"), names::TEST_SYSTEM, 100);

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(query_param("$expand", ".($levels=2)"))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(200).set_body_json(&test_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let expand_query = ExpandQuery::current().levels(2);
        let result = bmc.expand::<TestResource>(&resource_id, expand_query).await;

        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.name, names::TEST_SYSTEM);
        assert_eq!(retrieved.value, 100);
    }

    #[tokio::test]
    async fn test_get_request_with_filter() {
        let mock_server = MockServer::start().await;
        let resource_path = paths::SYSTEMS_1;

        let test_resource =
            create_test_resource(resource_path, Some("789"), names::TEST_SYSTEM, 50);

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(query_param("$filter", "value gt 10"))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(200).set_body_json(&test_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let filter_query = FilterQuery::gt(&"value", 10);
        let result = bmc.filter::<TestResource>(&resource_id, filter_query).await;

        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.name, names::TEST_SYSTEM);
        assert_eq!(retrieved.value, 50);
    }

    #[tokio::test]
    async fn test_post_create_request() {
        let mock_server = MockServer::start().await;
        let collection_path = paths::SYSTEMS_1;

        let create_request = CreateRequest {
            name: names::TEST_SYSTEM.to_string(),
            value: 999,
        };

        let created_resource =
            create_test_resource("/redfish/v1/systems/new", None, names::TEST_SYSTEM, 999);

        Mock::given(method("POST"))
            .and(path(collection_path))
            .and(body_json(&create_request))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(201).set_body_json(&created_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let collection_id = create_odata_id(collection_path);
        let result = bmc
            .create::<CreateRequest, TestResource>(&collection_id, &create_request)
            .await;

        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.name, names::TEST_SYSTEM);
        assert_eq!(created.value, 999);
    }

    #[tokio::test]
    async fn test_patch_update_request() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/systems/1";

        let update_request = UpdateRequest {
            name: Some("Updated System".to_string()),
            value: None,
        };

        let etag = create_odata_etag("abc123");

        let updated_resource = TestResource {
            id: create_odata_id(resource_path),
            etag: None,
            name: "Updated System".to_string(),
            value: 42,
        };

        Mock::given(method("PATCH"))
            .and(path(resource_path))
            .and(body_json(&update_request))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("If-Match", "abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&updated_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path(resource_path))
            .and(body_json(&update_request))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("If-Match", "*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&updated_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let result = bmc
            .update::<UpdateRequest, TestResource>(&resource_id, Some(&etag), &update_request)
            .await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.name, "Updated System");
        assert_eq!(updated.value, 42);

        let no_etag = bmc
            .update::<UpdateRequest, TestResource>(&resource_id, None, &update_request)
            .await;

        assert!(no_etag.is_ok());
    }

    #[tokio::test]
    async fn test_delete_request() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/systems/1";

        Mock::given(method("DELETE"))
            .and(path(resource_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.delete(&resource_id).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_action_request() {
        let mock_server = MockServer::start().await;
        let action_path = "/redfish/v1/systems/1/Actions/ComputerSystem.Reset";

        let action_request = ActionRequest {
            parameter: "ForceRestart".to_string(),
        };

        let action_response = ActionResponse {
            result: "Reset initiated".to_string(),
            success: true,
        };

        Mock::given(method("POST"))
            .and(path(action_path))
            .and(body_json(&action_request))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .respond_with(ResponseTemplate::new(200).set_body_json(&action_response))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let action = create_test_action(action_path);
        let result = bmc.action(&action, &action_request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.result, "Reset initiated");
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_get_request_4xx_error() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/nonexistent";

        Mock::given(method("GET"))
            .and(path(resource_path))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.get::<TestResource>(&resource_id).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, BmcError::InvalidResponse(_, _)));
    }

    #[tokio::test]
    async fn test_action_request_5xx_server_error() {
        let mock_server = MockServer::start().await;
        let action_path = "/redfish/v1/systems/1/Actions/ComputerSystem.Reset";

        let action_request = ActionRequest {
            parameter: "InvalidParameter".to_string(),
        };

        Mock::given(method("POST"))
            .and(path(action_path))
            .and(body_json(&action_request))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);

        let action = create_test_action(action_path);
        let result = bmc.action(&action, &action_request).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, BmcError::InvalidResponse(_, _)));
    }

    #[tokio::test]
    async fn test_custom_headers_in_get_request() {
        let mock_server = MockServer::start().await;
        let resource_path = paths::SYSTEMS_1;

        let test_resource =
            create_test_resource(resource_path, Some("123"), names::TEST_SYSTEM, 42);

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("X-Custom-Header", "custom-value"))
            .and(header("X-Auth-Token", "test-token-12345"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&test_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert("X-Custom-Header", "custom-value".parse().unwrap());
        custom_headers.insert("X-Auth-Token", "test-token-12345".parse().unwrap());

        let bmc = create_test_bmc_with_custom_headers(&mock_server, custom_headers);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.get::<TestResource>(&resource_id).await;

        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.name, names::TEST_SYSTEM);
        assert_eq!(retrieved.value, 42);
    }

    #[tokio::test]
    async fn test_custom_headers_in_post_request() {
        let mock_server = MockServer::start().await;
        let collection_path = paths::SYSTEMS_1;

        let create_request = CreateRequest {
            name: names::TEST_SYSTEM.to_string(),
            value: 999,
        };

        let created_resource =
            create_test_resource("/redfish/v1/systems/new", None, names::TEST_SYSTEM, 999);

        Mock::given(method("POST"))
            .and(path(collection_path))
            .and(body_json(&create_request))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("X-Vendor-Specific", "vendor-value"))
            .and(header("X-Request-Id", "req-123"))
            .respond_with(ResponseTemplate::new(201).set_body_json(&created_resource))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert("X-Vendor-Specific", "vendor-value".parse().unwrap());
        custom_headers.insert("X-Request-Id", "req-123".parse().unwrap());

        let bmc = create_test_bmc_with_custom_headers(&mock_server, custom_headers);

        let collection_id = create_odata_id(collection_path);
        let result = bmc
            .create::<CreateRequest, TestResource>(&collection_id, &create_request)
            .await;

        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.name, names::TEST_SYSTEM);
        assert_eq!(created.value, 999);
    }

    #[tokio::test]
    async fn test_custom_headers_in_delete_request() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/systems/1";

        Mock::given(method("DELETE"))
            .and(path(resource_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("X-Delete-Reason", "decommissioned"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert("X-Delete-Reason", "decommissioned".parse().unwrap());

        let bmc = create_test_bmc_with_custom_headers(&mock_server, custom_headers);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.delete(&resource_id).await;

        assert!(result.is_ok());
    }
}
