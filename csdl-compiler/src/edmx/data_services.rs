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

use crate::edmx::Schema;
use crate::edmx::ValidateError;
use serde::Deserialize;

/// 3.2 Element edmx:DataServices
#[derive(Debug, Deserialize)]
pub struct DeDataServices {
    /// edm:Schema elements which define the schemas exposed by the
    /// `OData` service
    #[serde(rename = "Schema", default)]
    pub schemas: Vec<Schema>,
}

/// Validated `DataServices`.
pub struct DataServices {
    pub schemas: Vec<Schema>,
}

impl DeDataServices {
    /// # Errors
    /// Validation error if any of Schemas is invalid.
    pub fn validate(self) -> Result<DataServices, ValidateError> {
        Ok(DataServices {
            schemas: self.schemas,
        })
    }
}
