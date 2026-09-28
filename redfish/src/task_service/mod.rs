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

//! Task Service entities and helpers.
//!
//! This module provides typed access to Redfish `TaskService`.
//! A `TaskService` value is a lightweight handle to the service schema and BMC
//! transport. It validates task locations returned by asynchronous operations
//! against this service's Tasks collection and returns lazy task links that can
//! be fetched when polling is needed.

use std::mem;
use std::sync::Arc;
use std::time::Duration;

use crate::core::ActionResult;
use crate::core::Bmc;
use crate::core::EntityTypeRef as _;
use crate::core::ModificationResponse;
use crate::core::NavProperty;
use crate::core::ODataId;
use crate::entity_link::EntityLink;
use crate::schema::task::Task as TaskSchema;
use crate::schema::task::TaskState;
use crate::schema::task_service::TaskService as TaskServiceSchema;
use crate::Error;
use crate::NvBmc;
use crate::ServiceRoot;

use nv_redfish_core::AsyncTask;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde::Deserializer;
use serde_json::Value as JsonValue;

/// Link to a Redfish Task returned by an asynchronous operation.
pub type TaskLink<B> = EntityLink<B, TaskSchema>;

enum State<R> {
    /// The result arrived with the original response.
    Ready(R),
    /// The operation is running.
    Pending(AsyncTask),
    /// The operation completed without returning its result.
    Finished,
    /// An earlier poll returned the outcome.
    Done,
}

/// Body of a Task Monitor response: the operation's result, or a Task
/// resource from a service that is polled at the Task instead.
enum MonitorBody<R> {
    Task(Box<TaskSchema>),
    Result(R),
}

impl<'de, R: ActionResult> Deserialize<'de> for MonitorBody<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = JsonValue::deserialize(deserializer)?;
        let is_task = value
            .get("@odata.type")
            .and_then(JsonValue::as_str)
            .and_then(|odata_type| odata_type.strip_prefix('#'))
            .is_some_and(|name| name.starts_with("Task."));
        if is_task {
            serde_json::from_value(value).map(|task| Self::Task(Box::new(task)))
        } else {
            serde_json::from_value(value).map(Self::Result)
        }
        .map_err(DeError::custom)
    }
}

/// Result of an action that may complete asynchronously.
///
/// The service returns the result immediately, or a Task Monitor or Task that
/// [`Self::poll_result`] follows until the typed result is available.
#[must_use = "asynchronous action results must be polled"]
pub struct AsyncActionResult<R: ActionResult> {
    state: State<R>,
}

impl<R: ActionResult> AsyncActionResult<R> {
    /// Wrap the response of an action that returns `R`.
    pub fn from_action_response(response: ModificationResponse<R>) -> Self {
        let state = match response {
            ModificationResponse::Entity(result) => State::Ready(result),
            ModificationResponse::Task(task) => State::Pending(task),
            ModificationResponse::Empty => State::Finished,
        };
        Self { state }
    }

    /// The Task Monitor or Task the next poll reads, while the operation is
    /// pending.
    ///
    /// A monitor can move the operation to a new URI, so persist this after
    /// each poll to resume later with [`Self::from_action_response`] and
    /// [`ModificationResponse::Task`].
    #[must_use]
    pub const fn pending_task(&self) -> Option<&AsyncTask> {
        match &self.state {
            State::Pending(task) => Some(task),
            State::Ready(_) | State::Finished | State::Done => None,
        }
    }

    /// Recommended delay before the next poll, from the latest pending
    /// response.
    ///
    /// `None` when the operation is not pending or the service sent no
    /// `Retry-After`.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match &self.state {
            State::Pending(task) => task.retry_after,
            State::Ready(_) | State::Finished | State::Done => None,
        }
    }

    /// Perform one polling step, returning the result once the operation
    /// completes and `None` while it is still running.
    ///
    /// A result read from a URI other than the Task Monitor can be left over
    /// from an earlier run, so callers should check request-specific data such
    /// as a nonce.
    ///
    /// # Errors
    ///
    /// Returns an error if a request fails, the Task ends without completing,
    /// the operation completes without exposing its result, or an earlier poll
    /// already returned the outcome. After a request error the same step can be
    /// retried.
    pub async fn poll_result<B: Bmc>(
        &mut self,
        task_service: &TaskService<B>,
    ) -> Result<Option<R>, Error<B>> {
        // The pending state is replaced only after a request completes, so a
        // dropped poll leaves the handle on the same step.
        let location = match &self.state {
            State::Pending(task) => task.location.0.clone(),
            State::Ready(_) | State::Finished | State::Done => {
                return match mem::replace(&mut self.state, State::Done) {
                    State::Ready(result) => Ok(Some(result)),
                    State::Finished => Err(Error::TaskResultUnavailable),
                    State::Pending(_) | State::Done => Err(Error::TaskAlreadyFinished),
                };
            }
        };

        let bmc = task_service.bmc.as_ref();
        let response = bmc
            .get_task_monitor_response::<MonitorBody<R>>(&location)
            .await
            .map_err(Error::Bmc)?;
        let task = match response {
            ModificationResponse::Task(pending) => {
                self.state = State::Pending(pending);
                return Ok(None);
            }
            ModificationResponse::Entity(MonitorBody::Result(result)) => {
                self.state = State::Done;
                return Ok(Some(result));
            }
            ModificationResponse::Empty => {
                self.state = State::Done;
                return Err(Error::TaskResultUnavailable);
            }
            ModificationResponse::Entity(MonitorBody::Task(task)) => task,
        };

        match task.task_state {
            Some(TaskState::Completed) => {
                // The result location is followed once; a second redirection
                // is reported as having no result.
                let Some(result_location) = recorded_location(&task) else {
                    self.state = State::Done;
                    return Err(Error::TaskResultUnavailable);
                };
                let response = bmc
                    .get_task_monitor_response::<R>(&result_location)
                    .await
                    .map_err(Error::Bmc)?;
                self.state = State::Done;
                match response {
                    ModificationResponse::Entity(result) => Ok(Some(result)),
                    ModificationResponse::Task(_) | ModificationResponse::Empty => {
                        Err(Error::TaskResultUnavailable)
                    }
                }
            }
            Some(state @ (TaskState::Exception | TaskState::Killed | TaskState::Cancelled)) => {
                self.state = State::Done;
                Err(Error::TaskFailed {
                    state,
                    messages: task.messages.unwrap_or_default(),
                })
            }
            _ => {
                self.state = State::Pending(AsyncTask {
                    location: location.into(),
                    retry_after: None,
                });
                Ok(None)
            }
        }
    }
}

/// The last `Location` a Task recorded in `Payload.HttpHeaders`, where
/// services without a Task Monitor report the result URI.
fn recorded_location(task: &TaskSchema) -> Option<ODataId> {
    task.payload
        .as_ref()?
        .http_headers
        .as_ref()?
        .iter()
        .rev()
        .find_map(|header| {
            let (name, location) = header.split_once(':')?;
            let location = location.trim();
            (name.trim().eq_ignore_ascii_case("Location") && !location.is_empty())
                .then(|| ODataId::from(location.to_string()))
        })
}

/// Task service.
///
/// Provides task links for task locations returned by asynchronous operations.
///
/// # Example
///
/// ```ignore
/// let Some(task_service) = root.task_service().await? else {
///     return Ok(());
/// };
///
/// let task_link = task_service.task_link(async_task)?;
/// let task = task_link.fetch().await?;
///
/// println!("{:?}", task.task_state);
/// ```
pub struct TaskService<B: Bmc> {
    data: Arc<TaskServiceSchema>,
    bmc: NvBmc<B>,
}

impl<B: Bmc> TaskService<B> {
    /// Create a new task service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        root: &ServiceRoot<B>,
    ) -> Result<Option<Self>, Error<B>> {
        let Some(service_ref) = &root.root.tasks else {
            return Ok(None);
        };

        let data = service_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

        // Task links need the BMC-advertised Tasks collection as the allowed
        // parent path for all async task locations.
        if data.tasks.is_none() {
            return Err(Error::TaskServiceTasksUnavailable);
        }

        Ok(Some(Self {
            data,
            bmc: bmc.clone(),
        }))
    }

    /// Get the raw schema data for this task service.
    #[must_use]
    pub fn raw(&self) -> Arc<TaskServiceSchema> {
        self.data.clone()
    }

    /// Create a task link from an asynchronous operation result.
    ///
    /// The task location must be a child of this service's Tasks collection,
    /// such as `/redfish/v1/TaskService/Tasks/{id}`. The returned link does not
    /// fetch the task until [`TaskLink::fetch`] is called.
    ///
    /// # Errors
    ///
    /// Returns error if the task location is not a child of this service's Tasks
    /// collection.
    pub fn task_link(&self, task: AsyncTask) -> Result<TaskLink<B>, Error<B>> {
        let Some(tasks) = self.data.tasks.as_ref() else {
            return Err(Error::TaskServiceTasksUnavailable);
        };

        let task_collection = tasks.odata_id();
        let task_location = task.location.0;
        if task_collection == &task_location || !task_collection.is_path_prefix(&task_location) {
            return Err(Error::TaskLocationNotInTaskService {
                task_location,
                task_collection: task_collection.clone(),
            });
        }

        let task_ref = NavProperty::new_reference(task_location);
        Ok(TaskLink::new(&self.bmc, task_ref))
    }
}
