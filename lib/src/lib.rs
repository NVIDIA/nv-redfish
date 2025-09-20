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

/// Type for `@odata.id` identifier.
pub mod bmc;
/// Type for navigation property.
pub mod nav_property;
/// Type for `@odata.id` identifier.
pub mod odata_id;

use serde::Deserialize;
use std::future::Future;

/// Reexport `Bmc` trait to make it available through crate root.
pub use bmc::Bmc;
/// Reexport `ODataId` to make it available through crate root.
pub type ODataId = odata_id::ODataId;
/// Reexport `NavProperty` to make it available through crate root.
pub type NavProperty<T> = nav_property::NavProperty<T>;

/// Entity type trait that is implemented by CSDL compiler for all
/// generated entity types.
pub trait EntityType {
    /// Value of `@odata.id` field of the Entity.
    fn id(&self) -> &ODataId;
}

pub trait Expandable: EntityType + Sized + for<'a> Deserialize<'a> {
    /// Expand entity type.
    fn expand<B: Bmc>(&self, bmc: &B) -> impl Future<Output = Result<Self, B::Error>> + Send {
        bmc.expand::<Self>(self.id())
    }
}
