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
//! transport. Each task read validates the supplied task path against this
//! service's Tasks collection, then performs a fresh GET and returns the
//! generated `Task` schema.

use std::sync::Arc;

use crate::core::Bmc;
use crate::core::EntityTypeRef as _;
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
/// let task = task_service.task(&async_task.id).await?;
///
/// println!("{:?}", task.task_state);
/// ```
pub struct TaskService<B: Bmc> {
    data: Arc<TaskServiceSchema>,
    task_collection: TaskCollectionPath,
    bmc: NvBmc<B>,
}

struct TaskCollectionPath {
    odata_id: ODataId,
    prefix: String,
}

impl<B: Bmc> TaskService<B> {
    /// Create a new task service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        root: &ServiceRoot<B>,
    ) -> Result<Option<Self>, Error<B>> {
        if let Some(service_ref) = &root.root.tasks {
            let data = service_ref.get(bmc.as_ref()).await.map_err(Error::Bmc)?;

            // Task polling needs the BMC-advertised Tasks collection as the
            // allowed parent path for all task reads.
            let Some(tasks) = data.tasks.as_ref() else {
                return Err(Error::TaskServiceTasksUnavailable);
            };

            let task_collection = tasks.odata_id().clone();
            let task_collection_prefix =
                format!("{}/", task_collection.to_string().trim_end_matches('/'));

            Ok(Some(Self {
                data,
                task_collection: TaskCollectionPath {
                    odata_id: task_collection,
                    prefix: task_collection_prefix,
                },
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

    /// Get a task directly by Redfish task path.
    ///
    /// The path must be under this service's Tasks collection, such as
    /// `/redfish/v1/TaskService/Tasks/{id}`.
    /// The returned generated schema exposes fields such as `task_state`,
    /// `task_status`, `percent_complete`, and `messages`.
    ///
    /// # Errors
    ///
    /// Returns error if the service does not expose a Tasks collection, if the
    /// path is outside that collection, or if retrieving task data fails.
    pub async fn task(&self, task_path: impl AsRef<str>) -> Result<Arc<Task>, Error<B>> {
        let task_path = task_path.as_ref();
        if !task_path.starts_with(self.task_collection.prefix.as_str()) {
            return Err(Error::TaskPathNotInTaskService {
                task_path: task_path.to_string().into(),
                task_collection: self.task_collection.odata_id.clone(),
            });
        }

        let task_ref = NavProperty::new_reference(ODataId::from(task_path.to_string()));
        task_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }
}

impl<B: Bmc> Resource for TaskService<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
