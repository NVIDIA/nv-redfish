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

//! HPE links advertised by a BIOS resource.

use std::sync::Arc;

use nv_redfish_core::Bmc;

use crate::oem::hpe::schema::hpe_bios_ext::HpeBiosExt as HpeBiosSchema;
use crate::oem::hpe::server_boot_settings::HpeServerBootSettings;
use crate::oem::oem_object;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::{Error, NvBmc};

/// HPE OEM extension to a BIOS resource.
pub struct HpeBios<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<HpeBiosSchema>,
}

impl<B: Bmc> HpeBios<B> {
    /// Parse the extension advertised under `Bios.Oem.Hpe`.
    pub(crate) fn new(bmc: &NvBmc<B>, oem: &ResourceOemSchema) -> Result<Option<Self>, Error<B>> {
        Ok(oem_object(oem, "Hpe")?.map(|data| Self {
            bmc: bmc.clone(),
            data,
        }))
    }

    /// Fetch the current HPE persistent boot-settings resource.
    ///
    /// The advertised link differs across iLO generations, so this method
    /// follows `Oem.Hpe.Links.Boot` without constructing a resource path.
    ///
    /// Returns `Ok(None)` when the link is absent or explicitly null.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching the boot-settings resource fails.
    pub async fn boot_settings(&self) -> Result<Option<HpeServerBootSettings<B>>, Error<B>> {
        let Some(boot) = self
            .data
            .links
            .as_ref()
            .and_then(|links| links.boot.as_ref())
        else {
            return Ok(None);
        };

        HpeServerBootSettings::new(&self.bmc, boot).await.map(Some)
    }

    /// Get the raw HPE BIOS extension.
    #[must_use]
    pub fn raw(&self) -> Arc<HpeBiosSchema> {
        self.data.clone()
    }
}
