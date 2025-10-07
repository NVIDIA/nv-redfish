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

use crate::Bmc;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use serde::Deserialize;
use serde::Serialize;
use std::marker::PhantomData;

/// Type for `target` field of Action.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ActionTarget(String);

impl ActionTarget {
    pub fn new(v: String) -> Self {
        Self(v)
    }
}

impl Display for ActionTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

#[derive(Deserialize, Debug)]
pub struct Action<T, R> {
    #[serde(rename = "target")]
    pub target: ActionTarget,
    // TODO: we can retrieve constrains on attributes here.
    #[serde(skip_deserializing)]
    pub _marker: PhantomData<T>,
    #[serde(skip_deserializing)]
    pub _marker_retval: PhantomData<R>,
}

pub trait ActionError {
    /// Create an error when action is not supported
    fn not_supported() -> Self;
}

impl<T: Send + Sync + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>> Action<T, R> {
    /// Run specific action with parameters passed as argument.
    pub async fn run<B: Bmc>(&self, bmc: &B, params: &T) -> Result<R, B::Error> {
        bmc.action::<T, R>(self, params).await
    }
}
