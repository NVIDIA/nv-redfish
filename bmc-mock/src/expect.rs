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

//! Expectations for Bmc Mock.

use nv_redfish_core::action::ActionTarget;
use nv_redfish_core::ODataId;
use serde_json::from_str;
use serde_json::Value as JsonValue;
use std::fmt::Display;

pub type Response<E> = Result<JsonValue, E>;

/// Request expected by BMC.
#[derive(Debug)]
pub enum ExpectedRequest {
    /// Expected Get.
    Get { id: ODataId },
    /// Expected Expand.
    Expand { id: ODataId },
    /// Expected Update.
    Update { id: ODataId, request: JsonValue },
    /// Expected Create.
    Create { id: ODataId, request: JsonValue },
    /// Expected ActionTarget
    Action {
        target: ActionTarget,
        request: JsonValue,
    },
    /// Expected Delete.
    Delete { id: ODataId },
    /// Expected Stream.
    Stream { uri: String },
}

/// Expectation for the tests.
#[derive(Debug)]
pub struct Expect<E> {
    pub request: ExpectedRequest,
    pub response: Response<E>,
}

impl<E> Expect<E> {
    pub fn get(uri: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Get {
                id: uri.to_string().into(),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }
    pub fn expand(uri: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Expand {
                id: uri.to_string().into(),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }
    pub fn update(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Update {
                id: uri.to_string().into(),
                request: from_str(&request.to_string()).expect("invalid json"),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }
    pub fn create(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Create {
                id: uri.to_string().into(),
                request: from_str(&request.to_string()).expect("invalid json"),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }
    pub fn action(uri: impl Display, request: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Action {
                target: ActionTarget::new(uri.to_string()),
                request: from_str(&request.to_string()).expect("invalid json"),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }

    pub fn delete(uri: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Delete {
                id: uri.to_string().into(),
            },
            response: Ok(JsonValue::Null),
        }
    }

    pub fn stream(uri: impl Display, response: impl Display) -> Self {
        Expect {
            request: ExpectedRequest::Stream {
                uri: uri.to_string(),
            },
            response: Ok(from_str(&response.to_string()).expect("invalid json")),
        }
    }
}
