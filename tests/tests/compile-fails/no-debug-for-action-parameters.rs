// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use nv_redfish_tests::base::redfish::service_root::TestActionsServiceTestSerializationActionAction;
use std::fmt::Debug;

// This is intentionally a compile-fail test. Action parameters can contain passwords, keys,
// tokens, or other sensitive values, but the schema compiler cannot currently identify which
// parameters need redaction. Requiring Debug here must therefore fail, protecting the generated
// action parameter type from accidentally gaining an unsafe derived Debug implementation.
fn require_debug<T: Debug>() {}

fn main() {
    // If this starts compiling, action parameters could be written to logs without redaction.
    // Debug support should only be enabled after sensitive parameters are explicitly identified.
    require_debug::<TestActionsServiceTestSerializationActionAction>();
}
