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

//! Integration tests of Account Service.

#![recursion_limit = "256"]

use nv_redfish::account::AccountCollection;
use nv_redfish::account::AccountService;
use nv_redfish::account::AccountTypes;
use nv_redfish::account::ManagerAccountCreate;
use nv_redfish::ServiceRoot;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use nv_redfish_tests::json_merge;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::test;

const ACCOUNT_SERVICE_DATA_TYPE: &str = "#AccountService.v1_5_0.AccountService";
const ACCOUNTS_DATA_TYPE: &str = "#ManagerAccountCollection.ManagerAccountCollection";
const MANAGER_ACCOUNT_DATA_TYPE: &str = "#ManagerAccount.v1_3_0.ManagerAccount";

#[test]
async fn list_accounts() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Contoso").await?;
    let maccount_id = format!("{}/Accounts/1", account_service.raw().odata_id());
    let accounts = get_account_collection(
        bmc.clone(),
        &account_service,
        json! {[{
            ODATA_ID: maccount_id,
            ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
            "Id": "1",
            "Name": "User Account",
            "UserName": "Administrator",
            "RoleId": "AdministratorRole",
            "AccountTypes": []
        }]},
    )
    .await?;
    let accounts = accounts.all_accounts_data().await?;
    assert_eq!(accounts.len(), 1);
    let account = accounts.first().unwrap().raw();
    assert_eq!(account.user_name, Some("Administrator".into()));
    assert_eq!(account.role_id, Some("AdministratorRole".into()));
    assert_eq!(account.base.name, "User Account");
    assert_eq!(account.base.id, "1");
    Ok(())
}

#[test]
async fn list_hpe_accounts() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "HPE").await?;
    let maccount_id = format!("{}/Accounts/1", account_service.raw().odata_id());
    let accounts = get_account_collection(
        bmc.clone(),
        &account_service,
        json! {[{
            ODATA_ID: maccount_id,
            ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
            "Id": "1",
            "Name": "User Account",
            "UserName": "Administrator",
            "RoleId": "AdministratorRole",
        }]},
    )
    .await?;
    let accounts = accounts.all_accounts_data().await?;
    assert_eq!(accounts.len(), 1);
    let account = accounts.first().unwrap().raw();
    assert_eq!(account.user_name, Some("Administrator".into()));
    assert_eq!(account.account_types, Some(vec![AccountTypes::Redfish]));
    Ok(())
}

#[test]
async fn list_no_patch_accounts() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Contoso").await?;
    let maccount_id = format!("{}/Accounts/1", account_service.raw().odata_id());
    assert!(get_account_collection(
        bmc.clone(),
        &account_service,
        json! {[{
            ODATA_ID: maccount_id,
            ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
            "Id": "1",
            "Name": "User Account",
            "UserName": "Administrator",
            "RoleId": "AdministratorRole",
        }]},
    )
    .await
    .is_err());
    Ok(())
}

async fn get_account_service(
    bmc: Arc<Bmc>,
    root_id: &ODataId,
    vendor: &str,
) -> Result<AccountService<Bmc>, Box<dyn StdError>> {
    let account_service_id = format!("{root_id}/AccountService");
    let data_type = "#ServiceRoot.v1_13_0.ServiceRoot";
    bmc.expect(Expect::get(
        &root_id,
        json!({
            ODATA_ID: &root_id,
            ODATA_TYPE: &data_type,
            "Id": "RootService",
            "Name": "RootService",
            "ProtocolFeaturesSupported": {
                "ExpandQuery": {
                    "NoLinks": true
                }
            },
            "AccountService": {
                ODATA_ID: &account_service_id,
            },
            "Vendor": vendor,
            "Links": {},
        }),
    ));
    let service_root = ServiceRoot::new(bmc.clone()).await?;

    let accounts_id = format!("{account_service_id}/Accounts");
    bmc.expect(Expect::get(
        &account_service_id,
        json!({
            ODATA_ID: &account_service_id,
            ODATA_TYPE: &ACCOUNT_SERVICE_DATA_TYPE,
            "Id": "AccountService",
            "Name": "AccountService",
            "Accounts": {
                ODATA_ID: &accounts_id,
            },
        }),
    ));
    Ok(service_root.account_service().await?.unwrap())
}

async fn get_account_collection(
    bmc: Arc<Bmc>,
    account_service: &AccountService<Bmc>,
    members: JsonValue,
) -> Result<AccountCollection<Bmc>, Box<dyn StdError>> {
    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    bmc.expect(Expect::expand(
        &accounts_id,
        json!({
            ODATA_ID: &accounts_id,
            ODATA_TYPE: &ACCOUNTS_DATA_TYPE,
            "Name": "User Accounts",
            "Members": members,
        }),
    ));
    Ok(account_service.accounts().await.map(Option::unwrap)?)
}

fn slot_member(accounts_id: &str, id: u32, enabled: bool, user_name: &str) -> JsonValue {
    json!({
        ODATA_ID: format!("{accounts_id}/{id}"),
        ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
        "Id": id.to_string(),
        "Name": "User Account",
        "Enabled": enabled,
        "AccountTypes": [],
        "UserName": user_name,
    })
}

// Create account (standard vendor): request includes required fields, response
// provides `AccountTypes: []` without patching.
#[test]
async fn create_account_standard() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Contoso").await?;
    let accounts = get_account_collection(bmc.clone(), &account_service, json!([])).await?;
    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    let maccount_id = format!("{accounts_id}/1");
    let create_req =
        ManagerAccountCreate::builder("password".into(), "user".into(), "Operator".into()).build();
    let create_json = serde_json::to_value(&create_req).unwrap();
    bmc.expect(Expect::create(
        &accounts_id,
        create_json,
        json!({
            ODATA_ID: maccount_id,
            ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
            "Id": "1",
            "Name": "User Account",
            "UserName": "user",
            "RoleId": "Operator",
            "AccountTypes": []
        }),
    ));
    let account = accounts.create_account(create_req).await?;
    let account = account.raw();
    assert_eq!(account.user_name, Some("user".into()));
    assert_eq!(account.role_id, Some("Operator".into()));
    assert_eq!(account.base.id, "1");
    assert_eq!(account.base.name, "User Account");
    assert!(account.account_types.as_ref().is_some_and(Vec::is_empty));
    Ok(())
}

// Create account (HPE-like vendor): response omits `AccountTypes`, expect
// defaulting to `[Redfish]` via read patching.
#[test]
async fn create_account_hpe_patched() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "HPE").await?;
    let accounts = get_account_collection(bmc.clone(), &account_service, json!([])).await?;
    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    let maccount_id = format!("{accounts_id}/1");
    let create_req =
        ManagerAccountCreate::builder("password".into(), "user".into(), "Operator".into()).build();
    let create_json = serde_json::to_value(&create_req).unwrap();
    bmc.expect(Expect::create(
        &accounts_id,
        create_json,
        json!({
            ODATA_ID: maccount_id,
            ODATA_TYPE: MANAGER_ACCOUNT_DATA_TYPE,
            "Id": "1",
            "Name": "User Account",
            "UserName": "user",
            "RoleId": "Operator"
        }),
    ));
    let account = accounts.create_account(create_req).await?;
    let account = account.raw();
    assert_eq!(account.user_name, Some("user".into()));
    assert_eq!(account.account_types, Some(vec![AccountTypes::Redfish]));
    Ok(())
}

// Create account (Dell slot-defined): choose first disabled slot with id >= min_slot (3).
#[test]
async fn create_account_dell_slot_defined_first_available() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Dell").await?;

    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    let members = json!([
        slot_member(&accounts_id, 1, true, "root"), // below min_slot, enabled
        slot_member(&accounts_id, 2, false, ""),    // below min_slot, disabled (must be skipped)
        slot_member(&accounts_id, 3, false, ""),    // first eligible slot
        slot_member(&accounts_id, 4, false, ""),
    ]);
    let accounts = get_account_collection(bmc.clone(), &account_service, members).await?;

    // Expect update on slot 3 with create params + enable.
    let update_req = nv_redfish::account::ManagerAccountUpdate::builder()
        .with_user_name("user".into())
        .with_password("password".into())
        .with_role_id("Operator".into())
        .with_enabled(true)
        .build();
    let update_json = serde_json::to_value(&update_req).unwrap();
    let maccount_id = format!("{accounts_id}/3");
    bmc.expect(Expect::update(
        &maccount_id,
        update_json,
        json_merge([
            &slot_member(&accounts_id, 3, true, "user"),
            &json! {{"RoleId": "Operator"}},
        ]),
    ));

    let create_req =
        ManagerAccountCreate::builder("password".into(), "user".into(), "Operator".into()).build();
    let account = accounts.create_account(create_req).await?;
    let account = account.raw();
    assert_eq!(account.base.id, "3");
    assert_eq!(account.user_name, Some("user".into()));
    assert_eq!(account.role_id, Some("Operator".into()));
    assert_eq!(account.enabled, Some(true));
    Ok(())
}

// Create account (Dell slot-defined): error when no disabled slot id >= min_slot is available.
#[test]
async fn create_account_dell_slot_defined_no_slot_available() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Dell").await?;

    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    // All eligible (>=3) are enabled; no disabled slots available.
    let members = json!([
        slot_member(&accounts_id, 1, false, ""),
        slot_member(&accounts_id, 2, false, ""),
        slot_member(&accounts_id, 3, true, "root"),
        slot_member(&accounts_id, 4, true, "other"),
    ]);
    let accounts = get_account_collection(bmc.clone(), &account_service, members).await?;

    let create_req =
        ManagerAccountCreate::builder("password".into(), "user".into(), "Operator".into()).build();
    assert!(accounts.create_account(create_req).await.is_err());
    Ok(())
}

// List accounts (Dell slot-defined): disabled accounts are hidden.
#[test]
async fn list_dell_accounts_hide_disabled() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let root_id = ODataId::service_root();
    let account_service = get_account_service(bmc.clone(), &root_id, "Dell").await?;

    let accounts_id = format!("{}/Accounts", account_service.raw().odata_id());
    let members = json!([
        slot_member(&accounts_id, 1, true, "root"),
        slot_member(&accounts_id, 3, false, ""),
        slot_member(&accounts_id, 4, true, "other"),
    ]);
    let accounts = get_account_collection(bmc.clone(), &account_service, members).await?;
    let data = accounts.all_accounts_data().await?;
    let ids: Vec<_> = data
        .into_iter()
        .map(|a| a.raw().as_ref().base.id.clone())
        .collect();
    assert_eq!(ids, vec!["1", "4"]);
    Ok(())
}
