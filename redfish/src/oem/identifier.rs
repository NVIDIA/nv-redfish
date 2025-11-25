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

use tagged_types::TaggedType;

/// OEM Identifier as it described in 9.8.3 (9.8.3 OEM-specified
/// object naming) of Redfish specification.
///
/// This identifier does not provide default comparison and Hash
/// because weird comparison rules defined by sepcification. This
/// should be partial case-insensitive, partially it can be
/// case-sensitive. So, in this library we give up to provide correct
/// code of universal comparison of OEM identifiers.
pub type Identifier<T> = TaggedType<T, IdentifierTag>;
#[doc(hidden)]
#[derive(tagged_types::Tag)]
// Do not add Eq, PartialEq, Ord, PartialOrd, Hash here. See doc above.
#[implement(Clone, Copy)]
#[transparent(Debug, Display, Serialize, Deserialize)]
#[capability(inner_access, cloned)]
pub enum IdentifierTag {}
