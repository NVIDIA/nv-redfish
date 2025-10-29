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

//! Log Service entities and collections.
//!
//! This module provides types for working with Redfish LogService resources
//! and their log entries.

use std::sync::Arc;

use nv_redfish_core::{http::ExpandQuery, Bmc, Expandable};

use crate::schema::redfish::log_entry::LogEntry;
use crate::schema::redfish::log_service::LogService as LogServiceSchema;
use crate::Error;

/// Log service.
///
/// Provides functions to access log entries and perform log operations.
pub struct LogService<B: Bmc> {
    bmc: Arc<B>,
    data: Arc<LogServiceSchema>,
}

impl<B: Bmc + Sync + Send> LogService<B> {
    /// Create a new log service handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<LogServiceSchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this log service.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<LogServiceSchema> {
        self.data.clone()
    }

    /// List all log entries.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The log service does not have a log entries collection
    /// - Fetching log entries data fails
    pub async fn list_entries(&self) -> Result<Vec<Arc<LogEntry>>, Error<B>> {
        let entries_ref = self
            .data
            .entries
            .as_ref()
            .ok_or(Error::LogEntriesNotAvailable)?;

        let entries_collection = entries_ref
            .expand(self.bmc.as_ref(), ExpandQuery::all())
            .await
            .map_err(Error::Bmc)?
            .get(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)?;

        let mut entries = Vec::new();
        for entry_ref in &entries_collection.members {
            let entry = entry_ref.get(self.bmc.as_ref()).await.map_err(Error::Bmc)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Clear all log entries.
    ///
    /// # Arguments
    ///
    /// * `log_entry_codes` - Optional log entry codes to clear specific entries
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The log service does not support the ClearLog action
    /// - The action execution fails
    pub async fn clear_log(&self, log_entry_codes: Option<String>) -> Result<(), Error<B>>
    where
        B::Error: nv_redfish_core::ActionError,
    {
        let actions = self
            .data
            .actions
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;

        actions
            .clear_log(self.bmc.as_ref(), log_entry_codes)
            .await
            .map_err(Error::Bmc)?;

        Ok(())
    }
}

