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

use crate::edmx::Namespace;
use serde::Deserialize;

/// 3.4 Element edmx:Include
#[derive(Debug, Deserialize)]
pub struct Include {
    /// 3.4.1 Attribute Namespace
    #[serde(rename = "@Namespace")]
    pub namespace: Namespace,
    /// 3.4.2 Attribute Alias
    #[serde(rename = "@Alias")]
    pub alias: Option<String>,
}
