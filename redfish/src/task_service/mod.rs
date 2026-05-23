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
//! transport. Each `TaskService::task` call performs a fresh GET for the
//! supplied Redfish task path and returns the generated `Task` schema.

use std::sync::Arc;

use crate::core::Bmc;
use crate::core::NavProperty;
use crate::core::ODataId;
use crate::schema::task_service::TaskService as TaskServiceSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use crate::ServiceRoot;

#[doc(inline)]
pub use crate::schema::task::Task;

/// Task service.
///
/// Provides direct task reads for task paths returned by asynchronous
/// operations.
///
/// # Example
///
/// ```ignore
/// let Some(task_service) = root.task_service().await? else {
///     return Ok(());
/// };
///
/// let task = task_service
///     .task("/redfish/v1/TaskService/Tasks/42")
///     .await?;
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
        if let Some(service_ref) = &root.root.tasks {
            let data = service_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

            Ok(Some(Self {
                data,
                bmc: bmc.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the raw schema data for this task service.
    #[must_use]
    pub fn raw(&self) -> Arc<TaskServiceSchema> {
        self.data.clone()
    }

    /// Get a task directly by Redfish path.
    ///
    /// Use this when an asynchronous response already returned a task path.
    /// The path should be in the form `/redfish/v1/TaskService/Tasks/{id}`.
    /// The returned generated schema exposes fields such as `task_state`,
    /// `task_status`, `percent_complete`, and `messages`.
    ///
    /// # Errors
    ///
    /// Returns error if retrieving task data fails.
    pub async fn task(&self, task_path: impl ToString) -> Result<Arc<Task>, Error<B>> {
        let task_ref = NavProperty::new_reference(ODataId::from(task_path.to_string()));
        task_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }
}

impl<B: Bmc> Resource for TaskService<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
