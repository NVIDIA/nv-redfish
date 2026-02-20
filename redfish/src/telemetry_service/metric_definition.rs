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

use crate::schema::redfish::metric_definition::MetricDefinition as MetricDefinitionSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef as _;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

pub use crate::schema::redfish::metric_definition::MetricDefinitionCreate;
pub use crate::schema::redfish::metric_definition::MetricDefinitionUpdate;

/// Metric definition entity wrapper.
pub struct MetricDefinition<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<MetricDefinitionSchema>,
}

impl<B: Bmc> MetricDefinition<B> {
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<MetricDefinitionSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    pub(crate) fn from_data(bmc: NvBmc<B>, data: MetricDefinitionSchema) -> Self {
        Self {
            bmc,
            data: Arc::new(data),
        }
    }

    /// Get raw metric definition schema data.
    #[must_use]
    pub fn raw(&self) -> Arc<MetricDefinitionSchema> {
        self.data.clone()
    }

    /// Update this metric definition.
    ///
    /// # Errors
    ///
    /// Returns an error if updating the entity fails.
    pub async fn update(&self, update: &MetricDefinitionUpdate) -> Result<Self, Error<B>> {
        let updated = self
            .bmc
            .as_ref()
            .update(self.data.id(), self.data.etag(), update)
            .await
            .map_err(Error::Bmc)?;
        Ok(Self::from_data(self.bmc.clone(), updated))
    }

    /// Delete this metric definition.
    ///
    /// # Errors
    ///
    /// Returns an error if deleting the entity fails.
    pub async fn delete(&self) -> Result<(), Error<B>> {
        self.bmc
            .as_ref()
            .delete(self.data.id())
            .await
            .map_err(Error::Bmc)
            .map(|_| ())
    }
}
