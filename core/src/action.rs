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

//! Redfish Action primitives
//!
//! The [`Action<T, R>`] type corresponds to the inner object found under the
//! Redfish `Actions` section for a specific action (for example,
//! `"#ComputerSystem.Reset"`). It captures the endpoint used to invoke the
//! action via its `target` field. The type parameters are:
//! - `T`: request parameters payload type (sent as the POST body when running the action)
//! - `R`: response type returned by the BMC for that action
//!
//! Only the `target` field is deserialized. Any additional metadata
//! (such as `...@Redfish.AllowableValues`) is ignored by this type
//! and may be used by higher layers.
//!
//! Example: how an action appears in a Redfish resource and which part maps to [`Action`]
//!
//! ```json
//! {
//!   "Actions": {
//!     "#ComputerSystem.Reset": {
//!       "target": "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset",
//!       "ResetType@Redfish.AllowableValues": [
//!         "On",
//!         "GracefulRestart",
//!         "ForceRestart"
//!       ]
//!     }
//!   }
//! }
//! ```
//!
//! The [`Action<T, R>`] value corresponds to the inner object of
//! `"#ComputerSystem.Reset"` and deserializes the `target` field only.
//!

use crate::Bmc;
use crate::ModificationResponse;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use serde::Deserialize;
use serde::Serialize;
use std::marker::PhantomData;

/// Type for the `target` field of an Action.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ActionTarget(String);

impl ActionTarget {
    /// Creates new `ActionTarget`.
    #[must_use]
    pub const fn new(v: String) -> Self {
        Self(v)
    }
}

impl Display for ActionTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

/// Defines a deserializable Action. It is almost always a member of the
/// `Actions` struct in different parts of the Redfish object tree.
///
/// `T` is the type for parameters.
/// `R` is the type for the return value.
#[derive(Deserialize, Debug)]
pub struct Action<T, R> {
    /// Path that is used to trigger the action.
    #[serde(rename = "target")]
    pub target: ActionTarget,
    // TODO: we can retrieve constraints on attributes here.
    /// Establishes a dependency on the `T` (parameters) type.
    #[serde(skip_deserializing)]
    _marker: PhantomData<T>,
    /// Establishes a dependency on the `R` (return value) type.
    #[serde(skip_deserializing)]
    _marker_retval: PhantomData<R>,
}

/// Action error trait. Needed in generated code when an action function
/// is called for an action that wasn't specified by the server.
pub trait ActionError {
    /// Create an error when the action is not supported.
    fn not_supported() -> Self;
}

impl<T: Send + Sync + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>> Action<T, R> {
    /// Run specific action with parameters passed as argument.
    ///
    /// # Errors
    ///
    /// Return error if BMC returned error on action.
    pub async fn run<B: Bmc>(
        &self,
        bmc: &B,
        params: &T,
    ) -> Result<ModificationResponse<R>, B::Error> {
        bmc.action::<T, R>(self, params).await
    }
}
