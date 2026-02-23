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

use crate::core::Bmc;
use crate::core::NavProperty;
pub use crate::oem::lenovo::schema::redfish::lenovo_security_service::FwRollbackState;
use crate::oem::lenovo::schema::redfish::lenovo_security_service::LenovoSecurityService as LenovoSecurityServiceSchema;
use crate::Error;
use crate::NvBmc;
use std::convert::identity;
use std::marker::PhantomData;
use std::sync::Arc;

/// Dell OEM Attributes.
pub struct LenovoSecurityService<B: Bmc> {
    data: Arc<LenovoSecurityServiceSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> LenovoSecurityService<B> {
    /// Create Lenovo OEM security service.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<LenovoSecurityServiceSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Firmware rollback is enabled.
    pub fn fw_rollback(&self) -> Option<FwRollbackState> {
        self.data
            .configurator
            .as_ref()
            .and_then(Option::as_ref)
            .and_then(|v| v.fw_rollback)
            .and_then(identity)
    }
}
