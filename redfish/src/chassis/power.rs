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

use std::sync::Arc;

use nv_redfish_core::Bmc;

use crate::schema::redfish::power::Power as PowerSchema;

/// Legacy Power resource wrapper.
///
/// This represents the deprecated `Chassis/Power` resource used in older
/// Redfish implementations. For modern BMCs, prefer using direct sensor
/// links via [`crate::metrics::HasMetrics`] or the PowerSubsystem resource.
///
/// Note: This type intentionally does NOT implement [`crate::metrics::HasMetrics`]
/// to encourage explicit handling of legacy vs modern approaches.
pub struct Power<B: Bmc> {
    #[allow(dead_code)]
    bmc: Arc<B>,
    data: Arc<PowerSchema>,
}

impl<B> Power<B>
where
    B: Bmc + Sync + Send,
{
    /// Create a new power resource handle.
    pub(crate) fn new(bmc: Arc<B>, data: Arc<PowerSchema>) -> Self {
        Self { bmc, data }
    }

    /// Get the raw schema data for this power resource.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data. The schema contains arrays of power supplies,
    /// voltages, and power control information.
    #[must_use]
    pub fn raw(&self) -> Arc<PowerSchema> {
        self.data.clone()
    }
}

