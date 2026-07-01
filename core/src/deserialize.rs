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

use serde::Deserialize;
use serde::Deserializer;

/// Deserialize an optional nullable field. nv-redfish models these fields
/// with `Option<Option<T>>`, where `None` means "no field" and
/// `Some(None)` means the field is explicitly set to null.
///
/// # Errors
///
/// Returns an error if deserialization of the underlying type fails.
pub fn de_optional_nullable<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

/// Deserialize a required nullable field. nv-redfish models these fields
/// with `Option<T>`, where `None` means null.
///
/// # Errors
///
/// Returns an error if deserialization of the underlying type fails.
pub fn de_required_nullable<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Deserialize::deserialize(de)
}

/// Deserialize a `null` field as an empty array.
/// Some BMCs return `{"Members": null}` instead of `{"Members": []}`.
pub fn de_null_to_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}
