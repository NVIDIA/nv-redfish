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

use nv_redfish::EntityType;
use nv_redfish::NavProperty;
use nv_redfish::ODataId;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Error;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use nv_redfish_tests::base::expect_root;
use nv_redfish_tests::base::get_service_root;
use nv_redfish_tests::json_merge;

use serde_json::json;
use tokio::test;

// Check trivial service root retrieval and version read.
#[test]
async fn get_service_root_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    let redfish_version = "1.0.0";
    bmc.expect(Expect::get(
        root_id.clone(),
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
            "RedfishVersion": redfish_version,
        }),
    ));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.id(), &root_id);
    assert_eq!(service_root.redfish_version, Some(redfish_version.into()));
    Ok(())
}

// Check that nullable optional property is represent by
// Option<Option<T>> and implementation can distinguish `"field:
// null"` from absense of `field`.
#[test]
async fn optional_nullable_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    let property_name = "OptionalNullable";
    let root_id = ODataId::service_root();
    let root_json = json!({
        ODATA_ID: &root_id,
        ODATA_TYPE: &data_type,
    });
    bmc.expect(Expect::get(
        root_id.clone(),
        json_merge([
            &root_json,
            &json!({
                property_name: null,
            }),
        ]),
    ));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.optional_nullable, Some(None));

    let value = "Value".to_string();
    bmc.expect(Expect::get(
        root_id.clone(),
        json_merge([
            &root_json,
            &json!({
                property_name: &value,
            }),
        ]),
    ));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.optional_nullable, Some(Some(value)));

    bmc.expect(Expect::get(root_id.clone(), root_json));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.optional_nullable, None);
    Ok(())
}

// Check service with required property.
#[test]
async fn required_non_nullable_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestRequiredService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");

    bmc.expect(expect_root(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert!(matches!(
        service_root.test_required_service.as_ref(),
        Some(NavProperty::Reference(_))
    ));

    let value = "SomeValue".to_string();
    bmc.expect(Expect::get(
        &service_id,
        &json!({
            ODATA_ID: &service_id,
            ODATA_TYPE: &service_data_type,
            "Required": &value,
        }),
    ));

    let service = service_root
        .test_required_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_required_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert_eq!(service.required, value);
    Ok(())
}

// Check that nullable optional property is represent by
// Option<Option<T>> and implementation can distinguish `"field:
// null"` from absense of `field`.
#[test]
async fn required_nullable_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestRequiredNullableService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = "ServiceRoot.v1_0_0.{service_name}";
    bmc.expect(expect_root(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    assert!(matches!(
        service_root.test_required_nullable_service.as_ref(),
        Some(NavProperty::Reference(_))
    ));

    let service_tpl = json!({
        ODATA_ID: &service_id,
        ODATA_TYPE: &service_data_type,
    });

    bmc.expect(Expect::get(
        &service_id,
        json_merge([
            &service_tpl,
            &json!({
                "RequiredNullable": null,
            }),
        ]),
    ));
    let service = service_root
        .test_required_nullable_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_nullable_required_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert_eq!(service.required_nullable, None);

    let value = "SomeValue".to_string();
    bmc.expect(Expect::get(
        service_id.clone(),
        json_merge([
            &service_tpl,
            &json!({
                "RequiredNullable": &value,
            }),
        ]),
    ));
    let service = service.refresh(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service.required_nullable, Some(value));

    bmc.expect(Expect::get(service_id.clone(), &service_tpl));
    assert!(service.refresh(&bmc).await.map_err(Error::Bmc).is_err());
    Ok(())
}
