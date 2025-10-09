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

//! Sometimes Redfish implementations do not perfectly match the CSDL
//! specification. This module provides helpers to deal with that.

/// Redfish collection related patches.
mod collection;
/// Redfish payload patches.
mod payload;

#[doc(inline)]
pub use collection::CollectionWithPatch;
#[doc(inline)]
pub use collection::CreateWithPatch;
#[doc(inline)]
pub use payload::Payload;
#[doc(inline)]
pub use payload::UpdateWithPatch;
#[doc(inline)]
pub use serde_json::Value as JsonValue;

use std::sync::Arc;

/// Reference to a patch function. This function should transform a JSON
/// structure to a Redfish-compatible structure.
pub type ReadPatchFn = Arc<dyn Fn(JsonValue) -> JsonValue + Sync + Send>;
