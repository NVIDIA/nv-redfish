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

//! BMC trait definition

use serde::Deserialize;
use serde::Serialize;

use crate::Action;
use crate::EntityTypeRef;
use crate::Expandable;
use crate::ODataId;
use crate::http::ExpandQuery;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

/// BMC trait defined access to Board Management Controller using
/// Redfish protocol.
pub trait Bmc {
    /// BMC Error
    type Error;

    fn expand<T: Expandable>(
        &self,
        id: &ODataId,
        query: ExpandQuery,
    ) -> impl Future<Output = Result<Arc<T>, Self::Error>> + Send;

    fn get<T: EntityTypeRef + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
    ) -> impl Future<Output = Result<Arc<T>, Self::Error>> + Send;

    fn create<V: Sync + Send + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        id: &ODataId,
        query: &V,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;

    fn update<V: Sync + Send + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        id: &ODataId,
        query: &V,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;

    fn delete(&self, id: &ODataId) -> impl Future<Output = Result<crate::Empty, Self::Error>> + Send;

    fn action<T: Send + Sync + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        action: &Action<T, R>,
        params: &T,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;
}

#[derive(Clone)]
pub struct BmcCredentials {
    pub username: String,
    password: String,
}

impl BmcCredentials {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

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
