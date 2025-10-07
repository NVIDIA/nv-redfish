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

//! Expectations for the test.

use nv_redfish_core::ODataId;
use nv_redfish_core::action::ActionTarget;
use std::fmt::Display;

/// Expectation for the tests.
#[derive(Debug)]
pub enum Expect {
    /// Expectation of get of specific object
    Get {
        id: ODataId,
        response: serde_json::Value,
    },
    /// Expectation of update of specific object
    Update {
        id: ODataId,
        request: serde_json::Value,
        response: serde_json::Value,
    },
    /// Expectation of create of specific object
    Create {
        id: ODataId,
        request: serde_json::Value,
        response: serde_json::Value,
    },
    /// Expectation of action on specific target
    Action {
        target: ActionTarget,
        request: serde_json::Value,
        response: serde_json::Value,
    },
}

impl Expect {
    pub fn get(uri: impl Display, response: impl Display) -> Self {
        Expect::Get {
            id: uri.to_string().into(),
            response: serde_json::from_str(&response.to_string()).expect("invalid json"),
        }
    }
    pub fn update(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect::Update {
            id: uri.to_string().into(),
            request: serde_json::from_str(&request.to_string()).expect("invalid json"),
            response: serde_json::from_str(&response.to_string()).expect("invalid json"),
        }
    }
    pub fn create(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect::Create {
            id: uri.to_string().into(),
            request: serde_json::from_str(&request.to_string()).expect("invalid json"),
            response: serde_json::from_str(&response.to_string()).expect("invalid json"),
        }
    }
    pub fn action(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect::Action {
            target: ActionTarget::new(uri.to_string()),
            request: serde_json::from_str(&request.to_string()).expect("invalid json"),
            response: serde_json::from_str(&response.to_string()).expect("invalid json"),
        }
    }
}
