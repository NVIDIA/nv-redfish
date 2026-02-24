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

//! Support Supermicro KCS Interface OEM resource.

use crate::core::Bmc;
use crate::core::NavProperty;
use crate::oem::supermicro::schema::redfish::kcs_interface::KcsInterface as KcsInterfaceSchema;
use crate::Error;
use crate::NvBmc;
use std::marker::PhantomData;
use std::sync::Arc;

#[doc(inline)]
pub use crate::oem::supermicro::schema::redfish::kcs_interface::Privilege;

/// Supermicro KCS interface resource.
pub struct KcsInterface<B: Bmc> {
    data: Arc<KcsInterfaceSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> KcsInterface<B> {
    /// Create a Supermicro KCS interface wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching KCS interface data fails.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<KcsInterfaceSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this Supermicro KCS interface.
    #[must_use]
    pub fn raw(&self) -> Arc<KcsInterfaceSchema> {
        self.data.clone()
    }

    /// Privilege associated with this KCS interface.
    #[must_use]
    pub fn privilege(&self) -> Option<Privilege> {
        self.data.privilege
    }
}
