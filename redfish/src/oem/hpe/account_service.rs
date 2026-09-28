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

//! HPE AccountService request composition.

use crate::schema::account_service::AccountServiceUpdate;

use super::update::oem_update;

#[doc(inline)]
pub use crate::oem::hpe::schema::hpei_lo_account_service::HpeiLoAccountServiceUpdate as HpeAccountServiceUpdate;

/// Adds HPE OEM settings to a standard AccountService update.
pub trait HpeAccountServiceUpdateExt: Sized {
    /// Merge HPE policy settings while preserving other OEM values.
    ///
    /// Repeated calls replace the existing `Hpe` member but retain every
    /// other vendor member.
    ///
    /// # Errors
    ///
    /// Returns an error if the existing OEM payload is not an object or the
    /// HPE update cannot be serialized.
    fn with_oem_hpe(self, hpe_update: HpeAccountServiceUpdate) -> Result<Self, serde_json::Error>;
}

impl HpeAccountServiceUpdateExt for AccountServiceUpdate {
    fn with_oem_hpe(
        mut self,
        hpe_update: HpeAccountServiceUpdate,
    ) -> Result<Self, serde_json::Error> {
        self.oem = Some(oem_update(self.oem.take(), &hpe_update)?);
        Ok(self)
    }
}
