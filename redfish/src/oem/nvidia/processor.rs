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

//! Support NVIDIA `Processor` OEM extensions.

use crate::oem::declares;
use crate::oem::nvidia::schema::nvidia_processor::NvidiaGpu as NvidiaGpuSchema;
use crate::oem::nvidia::schema::nvidia_processor::NvidiaProcessor as NvidiaProcessorSchema;
use crate::oem::nvidia::OEM_KEY;
use crate::oem::oem_value;
use crate::schema::resource::Oem as ResourceOemSchema;
use crate::Error;
use nv_redfish_core::Bmc;
use serde::Deserialize as _;
use std::sync::Arc;

/// NVIDIA extension of a processor.
pub enum NvidiaProcessor {
    /// GPU properties, including the properties shared by NVIDIA processors.
    Gpu(Arc<NvidiaGpuSchema>),
    /// Properties common to NVIDIA processors with another or unknown type.
    Generic(Arc<NvidiaProcessorSchema>),
}

impl NvidiaProcessor {
    /// Read the extension out of a `Processor` OEM payload.
    ///
    /// Returns `Ok(None)` when the payload carries no NVIDIA object,
    /// including when it carries an explicit `null`.
    ///
    /// Unknown or absent `@odata.type` values use the generic shape so
    /// shared processor properties remain available.
    pub(crate) fn new<B: Bmc>(oem: &ResourceOemSchema) -> Result<Option<Self>, Error<B>> {
        let Some(nvidia) = oem_value(oem, OEM_KEY) else {
            return Ok(None);
        };
        let this = if declares(nvidia, "NvidiaProcessor", "NvidiaGPU") {
            Self::Gpu(Arc::new(
                NvidiaGpuSchema::deserialize(nvidia).map_err(Error::Json)?,
            ))
        } else {
            Self::Generic(Arc::new(
                NvidiaProcessorSchema::deserialize(nvidia).map_err(Error::Json)?,
            ))
        };
        Ok(Some(this))
    }
}
