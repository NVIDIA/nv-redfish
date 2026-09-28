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

//! Integration tests of asynchronous action result polling.

use std::error::Error as StdError;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use nv_redfish::core::ActionResult;
use nv_redfish::core::AsyncTask;
use nv_redfish::core::ModificationResponse;
use nv_redfish::core::ODataId;
use nv_redfish::schema::task::TaskState;
use nv_redfish::task_service::AsyncActionResult;
use nv_redfish::task_service::TaskService;
use nv_redfish::Error;
use nv_redfish::ServiceRoot;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tokio::test;

const TASK_SERVICE_PATH: &str = "/redfish/v1/TaskService";
const MONITOR_PATH: &str = "/redfish/v1/TaskService/Tasks/42/Monitor";
const TASK_PATH: &str = "/redfish/v1/TaskService/Tasks/92";
const RESULT_PATH: &str =
    "/redfish/v1/ComponentIntegrity/EROT_BIOS_0/Actions/SPDMGetSignedMeasurements/Data";

async fn setup() -> Result<(Arc<Bmc>, TaskService<Bmc>), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    bmc.expect(Expect::get(
        "/redfish/v1",
        json!({
            ODATA_ID: "/redfish/v1",
            ODATA_TYPE: "#ServiceRoot.v1_13_0.ServiceRoot",
            "Id": "RootService",
            "Name": "Root Service",
            "Tasks": {
                ODATA_ID: TASK_SERVICE_PATH
            },
            "Links": {
                "Sessions": {
                    ODATA_ID: "/redfish/v1/SessionService/Sessions"
                }
            }
        }),
    ));
    bmc.expect(Expect::get(
        TASK_SERVICE_PATH,
        json!({
            ODATA_ID: TASK_SERVICE_PATH,
            ODATA_TYPE: "#TaskService.v1_1_4.TaskService",
            "Id": "TaskService",
            "Name": "Task Service",
            "Tasks": {
                ODATA_ID: "/redfish/v1/TaskService/Tasks"
            }
        }),
    ));
    let root = ServiceRoot::new(bmc.clone()).await?;
    let task_service = root
        .task_service()
        .await?
        .ok_or_else(|| IoError::new(ErrorKind::NotFound, "expected task service"))?;
    Ok((bmc, task_service))
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(transparent)]
struct Evidence(Value);

impl ActionResult for Evidence {}

fn evidence(value: Value) -> Option<Evidence> {
    Some(Evidence(value))
}

fn task(state: &str, fields: Value) -> Value {
    nv_redfish_tests::json_merge([
        &json!({
            ODATA_ID: TASK_PATH,
            ODATA_TYPE: "#Task.v1_4_2.Task",
            "Id": "92",
            "Name": "Task 92",
            "TaskState": state
        }),
        &fields,
    ])
}

fn pending_at_task() -> ModificationResponse<Evidence> {
    ModificationResponse::Task(AsyncTask {
        location: ODataId::from(TASK_PATH.to_string()).into(),
        retry_after: None,
    })
}

fn pending() -> ModificationResponse<Evidence> {
    ModificationResponse::Task(AsyncTask {
        location: ODataId::from(MONITOR_PATH.to_string()).into(),
        retry_after: Some(Duration::from_secs(30)),
    })
}

#[test]
async fn poll_returns_immediate_entity_once() -> Result<(), Box<dyn StdError>> {
    let (_bmc, task_service) = setup().await?;
    let mut response = AsyncActionResult::from_action_response(ModificationResponse::Entity(
        Evidence(json!({"result": "immediate"})),
    ));

    assert_eq!(
        response.poll_result(&task_service).await?,
        evidence(json!({"result": "immediate"}))
    );
    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskAlreadyFinished)
    ));

    Ok(())
}

#[test]
async fn poll_replaces_retry_after_until_monitor_returns_result() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_pending(
        MONITOR_PATH,
        None,
        Some(Duration::from_secs(9)),
    ));
    bmc.expect(Expect::task_monitor_pending(MONITOR_PATH, None, None));
    bmc.expect(Expect::task_monitor_result(
        MONITOR_PATH,
        json!({"result": "monitor"}),
    ));
    let mut response = AsyncActionResult::from_action_response(pending());
    assert_eq!(response.retry_after(), Some(Duration::from_secs(30)));

    assert_eq!(response.poll_result(&task_service).await?, None);
    assert_eq!(response.retry_after(), Some(Duration::from_secs(9)));
    assert_eq!(response.poll_result(&task_service).await?, None);
    assert_eq!(response.retry_after(), None);
    assert_eq!(
        response.poll_result(&task_service).await?,
        evidence(json!({"result": "monitor"}))
    );

    Ok(())
}

#[test]
async fn poll_retries_same_step_after_request_error() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_status(MONITOR_PATH, 503));
    bmc.expect(Expect::task_monitor_result(
        MONITOR_PATH,
        json!({"result": "monitor"}),
    ));
    let mut response = AsyncActionResult::from_action_response(pending());

    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::Bmc(_))
    ));
    assert_eq!(
        response.poll_result(&task_service).await?,
        evidence(json!({"result": "monitor"}))
    );

    Ok(())
}

#[test]
async fn cancelling_poll_preserves_pending_step() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_wait(MONITOR_PATH));
    bmc.expect(Expect::task_monitor_result(
        MONITOR_PATH,
        json!({"result": "monitor"}),
    ));
    let mut response = AsyncActionResult::from_action_response(pending());

    let mut poll = Box::pin(response.poll_result(&task_service));
    assert!(matches!(futures_util::poll!(poll.as_mut()), Poll::Pending));
    drop(poll);

    assert_eq!(
        response.poll_result(&task_service).await?,
        evidence(json!({"result": "monitor"}))
    );

    Ok(())
}

#[test]
async fn poll_empty_monitor_without_result_source_is_unavailable() -> Result<(), Box<dyn StdError>>
{
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_empty(MONITOR_PATH));
    let mut response = AsyncActionResult::from_action_response(pending());

    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskResultUnavailable)
    ));

    Ok(())
}

#[test]
async fn poll_immediate_empty_is_unavailable() -> Result<(), Box<dyn StdError>> {
    let (_bmc, task_service) = setup().await?;
    let mut response =
        AsyncActionResult::<Evidence>::from_action_response(ModificationResponse::Empty);

    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskResultUnavailable)
    ));
    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskAlreadyFinished)
    ));

    Ok(())
}

#[test]
async fn poll_task_resource_follows_recorded_result_location() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_result(
        TASK_PATH,
        task("Running", json!({})),
    ));
    bmc.expect(Expect::task_monitor_result(
        TASK_PATH,
        task(
            "Completed",
            json!({ "Payload": { "HttpHeaders": [format!("Location: {RESULT_PATH}")] } }),
        ),
    ));
    bmc.expect(Expect::task_monitor_result(
        RESULT_PATH,
        json!({"result": "viking"}),
    ));
    let mut response = AsyncActionResult::from_action_response(pending_at_task());

    assert_eq!(response.poll_result(&task_service).await?, None);
    assert_eq!(
        response.poll_result(&task_service).await?,
        evidence(json!({"result": "viking"}))
    );

    Ok(())
}

#[test]
async fn pending_task_tracks_moved_monitor_for_resume() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    let moved_monitor = "/redfish/v1/TaskService/Tasks/42/Monitor?page=2";
    bmc.expect(Expect::task_monitor_pending(
        MONITOR_PATH,
        Some(moved_monitor),
        Some(Duration::from_secs(5)),
    ));
    let mut response = AsyncActionResult::from_action_response(pending());
    assert_eq!(
        response
            .pending_task()
            .map(|task| task.location.0.to_string()),
        Some(MONITOR_PATH.to_string())
    );

    assert_eq!(response.poll_result(&task_service).await?, None);
    let persisted = response
        .pending_task()
        .cloned()
        .ok_or("expected a pending task")?;
    assert_eq!(persisted.location.0.to_string(), moved_monitor);
    drop(response);

    bmc.expect(Expect::task_monitor_result(
        moved_monitor,
        json!({"result": "resumed"}),
    ));
    let mut resumed =
        AsyncActionResult::from_action_response(ModificationResponse::Task(persisted));
    assert_eq!(
        resumed.poll_result(&task_service).await?,
        evidence(json!({"result": "resumed"}))
    );
    assert!(resumed.pending_task().is_none());

    Ok(())
}

#[test]
async fn poll_completed_task_without_result_location_is_unavailable(
) -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_result(
        TASK_PATH,
        task("Completed", json!({})),
    ));
    let mut response = AsyncActionResult::from_action_response(pending_at_task());

    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskResultUnavailable)
    ));

    Ok(())
}

#[test]
async fn poll_failed_task_reports_typed_state_and_messages() -> Result<(), Box<dyn StdError>> {
    let (bmc, task_service) = setup().await?;
    bmc.expect(Expect::task_monitor_result(
        TASK_PATH,
        task(
            "Exception",
            json!({
                "Messages": [
                    {"MessageId": "SPDM.1.0.Timeout", "Message": "Responder timed out"}
                ]
            }),
        ),
    ));
    let mut response = AsyncActionResult::from_action_response(pending_at_task());

    let Err(Error::TaskFailed { state, messages }) = response.poll_result(&task_service).await
    else {
        return Err("expected a failed Task".into());
    };
    assert_eq!(state, TaskState::Exception);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message.as_deref(), Some("Responder timed out"));
    assert!(matches!(
        response.poll_result(&task_service).await,
        Err(Error::TaskAlreadyFinished)
    ));

    Ok(())
}
