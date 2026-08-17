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
//! Manager network protocol resource.

use std::marker::PhantomData;
use std::sync::Arc;

use nv_redfish_core::{Bmc, NavProperty};

use crate::schema::manager_network_protocol::ManagerNetworkProtocol as ManagerNetworkProtocolSchema;
use crate::{Error, NvBmc};

/// Network protocol configuration associated with a manager.
pub struct ManagerNetworkProtocol<B: Bmc> {
    data: Arc<ManagerNetworkProtocolSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> ManagerNetworkProtocol<B> {
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<ManagerNetworkProtocolSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for the manager network protocol resource.
    #[must_use]
    pub fn raw(&self) -> Arc<ManagerNetworkProtocolSchema> {
        self.data.clone()
    }
}
