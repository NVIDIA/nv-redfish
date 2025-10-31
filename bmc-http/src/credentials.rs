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

use std::fmt;

/// Credentials used to access the BMC.
///
/// Security notes:
/// - `Debug`/`Display` redact the password by design.
/// - Prefer short-lived instances and avoid logging credentials.
#[derive(Clone)]
pub struct BmcCredentials {
    /// Username to access BMC.
    pub username: String,
    password: String,
}

impl BmcCredentials {
    /// Create new credentials.
    #[must_use]
    pub const fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    /// Get password.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

impl fmt::Debug for BmcCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BmcCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Display for BmcCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BmcCredentials(username: {}, password: [REDACTED])",
            self.username
        )
    }
}
