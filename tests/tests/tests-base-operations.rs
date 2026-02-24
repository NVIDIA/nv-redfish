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

use nv_redfish_core::Creatable;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataId;
use nv_redfish_core::RedfishSettings;
use nv_redfish_core::Updatable;
use nv_redfish_tests::base::expect_root;
use nv_redfish_tests::base::expect_root_srv;
use nv_redfish_tests::base::get_service_root;
use nv_redfish_tests::base::nav_service_root;
use nv_redfish_tests::base::redfish::service_root::ActionType;
use nv_redfish_tests::base::redfish::service_root::ServiceRootUpdate;
use nv_redfish_tests::base::redfish::service_root::TestCollectionMemberCreate;
use nv_redfish_tests::json_merge;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Error;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
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
        json_merge([&root_json, &json!({ property_name: null })]),
    ));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.optional_nullable, Some(None));

    let value = "Value".to_string();
    bmc.expect(Expect::get(
        root_id.clone(),
        json_merge([&root_json, &json!({ property_name: &value })]),
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

    bmc.expect(expect_root_srv(service_name, &service_id));
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
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");
    let property_name = "RequiredNullable";
    bmc.expect(expect_root_srv(service_name, &service_id));
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
        json_merge([&service_tpl, &json!({ property_name: null })]),
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
        json_merge([&service_tpl, &json!({ property_name: &value })]),
    ));
    let service = service.refresh(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service.required_nullable, Some(value));

    bmc.expect(Expect::get(service_id.clone(), &service_tpl));
    assert!(service.refresh(&bmc).await.map_err(Error::Bmc).is_err());
    Ok(())
}

// Check that nullable optional property is represent by
// Option<Option<T>> and implementation can distinguish `"field:
// null"` from absense of `field`.
#[test]
async fn required_collection_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestRequiredCollectionService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");
    let property_name = "RequiredCollection";
    bmc.expect(expect_root_srv(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    assert!(matches!(
        service_root.test_required_collection_service.as_ref(),
        Some(NavProperty::Reference(_))
    ));

    let service_tpl = json!({
        ODATA_ID: &service_id,
        ODATA_TYPE: &service_data_type,
    });

    let empty = Vec::<String>::new();
    bmc.expect(Expect::get(
        &service_id,
        json_merge([&service_tpl, &json!({ property_name: empty })]),
    ));
    let service = service_root
        .test_required_collection_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_nullable_required_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert_eq!(service.required_collection, empty);

    let value = vec!["SomeValue1".to_string(), "SomeValue2".to_string()];
    bmc.expect(Expect::get(
        service_id.clone(),
        json_merge([&service_tpl, &json!({ property_name: value })]),
    ));
    let service = service.refresh(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service.required_collection, value);

    Ok(())
}

// Check updatable for properties.
#[test]
async fn update_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    let updatable_name = "Updatable";
    let updatable_guid_name = "UpdatableGuid";
    let write_only_name = "WriteOnly";
    let root_id = ODataId::service_root();
    let root_json = json!({
        ODATA_ID: &root_id,
        ODATA_TYPE: &data_type,
    });
    bmc.expect(expect_root());
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert_eq!(service_root.updatable, None);

    let uuid_str = "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8";
    let uuid_value = uuid_str.parse().expect("uuid must be parsed");
    let value = "Value".to_string();
    bmc.expect(Expect::update(
        root_id.clone(),
        json!({ updatable_name: &value, updatable_guid_name: &uuid_str }),
        &json_merge([
            &root_json,
            &json!({ updatable_name: &value, updatable_guid_name: &uuid_str}),
        ]),
    ));
    let service_root = service_root
        .update(
            &bmc,
            &ServiceRootUpdate {
                // Here we actually checks that update struct doesn't include:
                // 1. read-only fields (like redfish_version)
                // 2. fields of read-only complex types (like read_only_complex)
                //
                // If this code compiles then check passed.
                updatable: Some(value.clone()),
                updatable_guid: Some(uuid_value),
                write_only: None,
            },
        )
        .await
        .map_err(Error::Bmc)?;
    let service_root = match service_root {
        ModificationResponse::Entity(service_root) => service_root,
        _ => return Err(Error::ExpectedProperty("service_root")),
    };
    assert_eq!(service_root.updatable, Some(value));
    assert_eq!(service_root.updatable_guid, Some(uuid_value));

    // Update write only:
    let value = "Value".to_string();
    bmc.expect(Expect::update(
        root_id.clone(),
        json!({ write_only_name: &value }),
        &json_merge([&root_json, &json!({})]),
    ));
    service_root
        .update(
            &bmc,
            &ServiceRootUpdate {
                updatable: None,
                updatable_guid: None,
                write_only: Some(value.clone()),
            },
        )
        .await
        .map_err(Error::Bmc)?;
    Ok(())
}

// Check updatable for navigation property.
#[test]
async fn update_using_nav_property_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    let updatable_name = "Updatable";
    let root_id = ODataId::service_root();
    let root_json = json!({
        ODATA_ID: &root_id,
        ODATA_TYPE: &data_type,
    });
    bmc.expect(expect_root());
    let nav_service_root = nav_service_root();
    let value = "Value".to_string();
    bmc.expect(Expect::update(
        root_id.clone(),
        json!({ updatable_name: &value }),
        &json_merge([&root_json, &json!({ updatable_name: &value })]),
    ));
    let nav_service_root = nav_service_root
        .update(
            &bmc,
            &ServiceRootUpdate {
                updatable: Some(value.clone()),
                updatable_guid: None,
                write_only: None,
            },
        )
        .await
        .map_err(Error::Bmc)?;
    let nav_service_root = match nav_service_root {
        ModificationResponse::Entity(nav_service_root) => nav_service_root,
        _ => return Err(Error::ExpectedProperty("nav_service_root")),
    };
    assert_eq!(
        nav_service_root
            .get(&bmc)
            .await
            .expect("no requests created")
            .updatable,
        Some(value)
    );
    Ok(())
}

// Check that write only is not generated in read structures.
#[test]
async fn no_write_only_in_read_struct() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fails/no-write-only-in-read.rs");
}

// Check that collection provides create method.
#[test]
async fn create_collection_member_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let collection_name = "TestCollection";
    let collection_id = format!("{root_id}/{collection_name}");
    let collection_data_type = format!("ServiceRoot.v1_0_0.{collection_name}");
    bmc.expect(expect_root_srv(collection_name, &collection_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    assert!(matches!(
        service_root.test_collection.as_ref(),
        Some(NavProperty::Reference(_))
    ));

    let collection_tpl = json!({
        ODATA_ID: &collection_id,
        ODATA_TYPE: &collection_data_type,
    });
    bmc.expect(Expect::get(
        &collection_id,
        json_merge([&collection_tpl, &json!({ "Members": [] })]),
    ));
    let collection = service_root
        .test_collection
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_collection"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;

    let collection_member_id = format!("{root_id}/{collection_name}/1");
    let collection_member_data_type = format!("ServiceRoot.v1_0_0.TestCollectionMember");
    let collection_member_tpl = json!({
        ODATA_ID: &collection_member_id,
        ODATA_TYPE: &collection_member_data_type,
    });
    bmc.expect(Expect::create(
        &collection_id,
        json!({}),
        collection_member_tpl,
    ));
    let member = collection
        .create(&bmc, &TestCollectionMemberCreate {})
        .await
        .map_err(Error::Bmc)?;
    let member = match member {
        ModificationResponse::Entity(member) => member,
        _ => return Err(Error::ExpectedProperty("member")),
    };
    assert_eq!(member.id().to_string(), collection_member_id);
    Ok(())
}

// Check that actions method.
#[test]
async fn action_method_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestActionsService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");
    let action_field = format!("#{service_name}.TestAction");
    let action_target = format!("{root_id}/{service_name}/Actions/{service_name}.TestAction");
    bmc.expect(expect_root_srv(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    assert!(matches!(
        service_root.test_actions_service.as_ref(),
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
                "Actions": {
                    action_field: {
                        "target": action_target
                    }
                }
            }),
        ]),
    ));
    let service = service_root
        .test_actions_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_actions_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;

    let service_actions = service
        .actions
        .as_ref()
        .ok_or(Error::ExpectedProperty("actions"))?;

    bmc.expect(Expect::action(
        &action_target,
        &json!({
            "ActionType": "Option1"
        }),
        &json!(null),
    ));
    service_actions
        .test_action(&bmc, Some(ActionType::Option1))
        .await
        .map_err(Error::Bmc)?;

    Ok(())
}

// Deserialize @Redfish.Settings and navigate to settings object.
#[test]
async fn redfish_settings_nav_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestSettingsService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");

    bmc.expect(expect_root_srv(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    let settings_id = format!("{service_id}/Settings");
    bmc.expect(Expect::get(
        &service_id,
        json!({
            ODATA_ID: &service_id,
            ODATA_TYPE: &service_data_type,
            "@Redfish.Settings": { "SettingsObject": { ODATA_ID: &settings_id } },
            "@Redfish.SettingsApplyTime": {},
            "SettingValue": "current",
        }),
    ));
    let service = service_root
        .test_settings_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_settings_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;

    assert!(service.redfish_settings.is_some());
    assert!(service.redfish_settings_apply_type.is_some());
    let settings_nav = service.settings_object().expect("settings nav must exist");

    // Fetch settings object
    bmc.expect(Expect::get(
        &settings_id,
        json!({
            ODATA_ID: &settings_id,
            ODATA_TYPE: &service_data_type,
            "SettingValue": "current",
        }),
    ));
    let _settings = settings_nav.get(&bmc).await.map_err(Error::Bmc)?;
    Ok(())
}

// Update via settings object; ensure update goes to settings resource id and applies value.
#[test]
async fn redfish_settings_update_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestSettingsService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");

    bmc.expect(expect_root_srv(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    let settings_id = format!("{service_id}/Settings");
    bmc.expect(Expect::get(
        &service_id,
        json!({
            ODATA_ID: &service_id,
            ODATA_TYPE: &service_data_type,
            "@Redfish.Settings": { "SettingsObject": { ODATA_ID: &settings_id } },
            "@Redfish.SettingsApplyTime": {},
            "SettingValue": "current",
        }),
    ));
    let service = service_root
        .test_settings_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_settings_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;

    let settings_nav = service.settings_object().expect("settings nav must exist");
    // Retrieve settings object once and update it
    bmc.expect(Expect::get(
        &settings_id,
        json!({
            ODATA_ID: &settings_id,
            ODATA_TYPE: &service_data_type,
            "SettingValue": "current",
        }),
    ));
    let settings = settings_nav.get(&bmc).await.map_err(Error::Bmc)?;

    let new_value = "new".to_string();
    bmc.expect(Expect::update(
        &settings_id,
        json!({ "SettingValue": &new_value }),
        json!({
            ODATA_ID: &settings_id,
            ODATA_TYPE: &service_data_type,
            "SettingValue": &new_value,
        }),
    ));
    let updated = settings
        .update(
            &bmc,
            &nv_redfish_tests::base::redfish::service_root::TestSettingsServiceUpdate {
                setting_value: Some(new_value.clone()),
            },
        )
        .await
        .map_err(Error::Bmc)?;
    let updated = match updated {
        ModificationResponse::Entity(updated) => updated,
        _ => return Err(Error::ExpectedProperty("updated")),
    };
    assert_eq!(updated.setting_value, Some(Some(new_value)));
    Ok(())
}

// If no @Redfish.Settings present, settings_object() returns None.
#[test]
async fn redfish_settings_absent_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();
    let service_name = "TestSettingsService";
    let service_id = format!("{root_id}/{service_name}");
    let service_data_type = format!("ServiceRoot.v1_0_0.{service_name}");

    bmc.expect(expect_root_srv(service_name, &service_id));
    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;

    // No settings annotations included
    bmc.expect(Expect::get(
        &service_id,
        json!({
            ODATA_ID: &service_id,
            ODATA_TYPE: &service_data_type,
            "SettingValue": "current",
        }),
    ));
    let service = service_root
        .test_settings_service
        .as_ref()
        .ok_or(Error::ExpectedProperty("test_settings_service"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert!(service.settings_object().is_none());
    Ok(())
}

// Excerpt view tests: verify inline excerpt copies and direct read
#[test]
async fn excerpt_views_test() -> Result<(), Error> {
    let bmc = Bmc::default();
    let root_id = ODataId::service_root();

    // Expect root with links to new services (both in a single response)
    let data_type = "ServiceRoot.v1_0_0.ServiceRoot";
    let excerpt_entity_id = format!("{}/ExcerptEntity", root_id);
    let excerpt_ref_entity_id = format!("{}/ExcerptRefEntity", root_id);
    bmc.expect(Expect::get(
        root_id.clone(),
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
            "ExcerptEntity": { ODATA_ID: &excerpt_entity_id },
            "ExcerptRefEntity": { ODATA_ID: &excerpt_ref_entity_id },
        }),
    ));

    let service_root = get_service_root(&bmc).await.map_err(Error::Bmc)?;
    assert!(matches!(
        service_root.excerpt_entity.as_ref(),
        Some(NavProperty::Reference(_))
    ));
    assert!(matches!(
        service_root.excerpt_ref_entity.as_ref(),
        Some(NavProperty::Reference(_))
    ));

    // Fetch ExcerptRefEntity and validate inline excerpts
    let ref_id = format!("{}/ExcerptRefEntity", root_id);
    let ref_dt = "ServiceRoot.v1_0_0.ExcerptRefEntity";
    let all = json!({
      "Always": "A",
      "BasicProp": "B",
      "DetailsProp": "D"
    });
    let basic = json!({
      "Always": "A",
      "BasicProp": "B"
    });
    let details = json!({
      "Always": "A",
      "DetailsProp": "D"
    });

    bmc.expect(Expect::get(
        &ref_id,
        json!({
          ODATA_ID: &ref_id,
          ODATA_TYPE: ref_dt,
          "ExcerptAll": all,
          "ExcerptBasic": basic,
          "ExcerptDetails": details,
        }),
    ));
    let ref_svc = service_root
        .excerpt_ref_entity
        .as_ref()
        .ok_or(Error::ExpectedProperty("excerpt_ref_entity"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert_eq!(
        ref_svc.excerpt_all.as_ref().unwrap().always,
        Some("A".into())
    );
    assert_eq!(
        ref_svc.excerpt_basic.as_ref().unwrap().basic_prop,
        Some("B".into())
    );
    assert_eq!(
        ref_svc.excerpt_details.as_ref().unwrap().details_prop,
        Some("D".into())
    );

    // Fetch ExcerptEntity directly and verify full entity contains Hidden
    let tgt_id = format!("{}/ExcerptEntity", root_id);
    let tgt_dt = "ServiceRoot.v1_0_0.ExcerptEntity";
    bmc.expect(Expect::get(
        &tgt_id,
        json!({
          ODATA_ID: &tgt_id,
          ODATA_TYPE: tgt_dt,
          "Always": "A",
          "BasicProp": "B",
          "DetailsProp": "D",
          "Hidden": "H"
        }),
    ));
    let tgt = service_root
        .excerpt_entity
        .as_ref()
        .ok_or(Error::ExpectedProperty("excerpt_entity"))?
        .get(&bmc)
        .await
        .map_err(Error::Bmc)?;
    assert_eq!(tgt.hidden, Some("H".into()));

    Ok(())
}
