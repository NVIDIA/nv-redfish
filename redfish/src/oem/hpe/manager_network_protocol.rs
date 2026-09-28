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

//! HPE OEM extension to ManagerNetworkProtocol.

use std::sync::Arc;

use nv_redfish_core::{
    Bmc, EntityTypeRef as _, ModificationResponse, NavProperty, ODataETag, ODataId,
};

use crate::manager::ManagerNetworkProtocol;
use crate::oem::hpe::schema::hpei_lo_manager_network_service::HpeiLoManagerNetworkService as HpeManagerNetworkProtocolSchema;
use crate::oem::oem_object;
use crate::schema::manager_network_protocol::{
    ManagerNetworkProtocol as ManagerNetworkProtocolSchema, ManagerNetworkProtocolUpdate,
};
use crate::{Error, NvBmc};

use super::update::oem_update;

#[doc(inline)]
pub use crate::oem::hpe::schema::hpei_lo_manager_network_service::HpeiLoManagerNetworkServiceUpdate;

/// HPE OEM properties on a ManagerNetworkProtocol resource.
pub struct HpeManagerNetworkProtocol<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<HpeManagerNetworkProtocolSchema>,
    network_protocol_etag: Option<ODataETag>,
    network_protocol_id: ODataId,
}

impl<B: Bmc> HpeManagerNetworkProtocol<B> {
    /// Parse `ManagerNetworkProtocol.Oem.Hpe`.
    pub(crate) fn new(
        bmc: &NvBmc<B>,
        network_protocol: &ManagerNetworkProtocolSchema,
    ) -> Result<Option<Self>, Error<B>> {
        Ok(network_protocol
            .oem
            .as_ref()
            .map_or_else(|| Ok(None), |oem| oem_object(oem, "Hpe"))?
            .map(|data| Self {
                bmc: bmc.clone(),
                data,
                network_protocol_etag: network_protocol.etag().cloned(),
                network_protocol_id: network_protocol.odata_id().clone(),
            }))
    }

    /// Whether host-side KCS access is enabled.
    #[must_use]
    pub fn kcs_enabled(&self) -> Option<bool> {
        self.data.kcs_enabled
    }

    /// Update the inline HPE network-protocol extension.
    ///
    /// # Errors
    ///
    /// Returns an error if serializing the OEM payload, updating the resource,
    /// or fetching the returned resource fails.
    pub async fn update(
        &self,
        update: &HpeiLoManagerNetworkServiceUpdate,
    ) -> Result<ModificationResponse<ManagerNetworkProtocol<B>>, Error<B>> {
        let update = ManagerNetworkProtocolUpdate::builder()
            .with_oem(oem_update(None, update).map_err(Error::Json)?)
            .build();

        self.bmc
            .as_ref()
            .update::<_, NavProperty<ManagerNetworkProtocolSchema>>(
                &self.network_protocol_id,
                self.network_protocol_etag.as_ref(),
                &update,
            )
            .await
            .map_err(Error::Bmc)?
            .try_map_entity_async(|nav| async move {
                ManagerNetworkProtocol::new(&self.bmc, &nav).await
            })
            .await
    }

    /// Enable or disable host-side KCS access.
    ///
    /// Returns `Ok(None)` when `KcsEnabled` is absent or null.
    ///
    /// # Errors
    ///
    /// Returns an error if updating or fetching the returned resource fails.
    pub async fn set_kcs_enabled(
        &self,
        enabled: bool,
    ) -> Result<Option<ModificationResponse<ManagerNetworkProtocol<B>>>, Error<B>> {
        if self.data.kcs_enabled.is_none() {
            return Ok(None);
        }

        self.update(
            &HpeiLoManagerNetworkServiceUpdate::builder()
                .with_kcs_enabled(enabled)
                .build(),
        )
        .await
        .map(Some)
    }

    /// Get the raw HPE extension.
    #[must_use]
    pub fn raw(&self) -> Arc<HpeManagerNetworkProtocolSchema> {
        self.data.clone()
    }
}
