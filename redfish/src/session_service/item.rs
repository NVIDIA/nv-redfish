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

//! Redfish Session - high-level wrapper.

use crate::schema::session::Session as SessionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef as _;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::NavProperty;
use std::sync::Arc;

/// Represents a Redfish `Session`.
pub struct Session<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<SessionSchema>,
}

impl<B: Bmc> Session<B> {
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<SessionSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
            })
    }

    pub(crate) fn from_data(bmc: NvBmc<B>, data: SessionSchema) -> Self {
        Self {
            bmc,
            data: Arc::new(data),
        }
    }

    /// Get the raw schema data for this session.
    #[must_use]
    pub fn raw(&self) -> Arc<SessionSchema> {
        self.data.clone()
    }

    /// Delete the current session.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self) -> Result<Option<Self>, Error<B>> {
        match self
            .bmc
            .as_ref()
            .delete::<NavProperty<SessionSchema>>(self.data.odata_id())
            .await
            .map_err(Error::Bmc)?
        {
            ModificationResponse::Entity(nav) => Self::new(&self.bmc, &nav).await.map(Some),
            ModificationResponse::Task(_) | ModificationResponse::Empty => Ok(None),
        }
    }
}

impl<B: Bmc> Resource for Session<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
