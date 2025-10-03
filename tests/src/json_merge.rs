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

use serde_json::Map;
use serde_json::Value;

/// Deep-merge a sequence of JSON values. Later values win.
/// - If both are objects: merge recursively
/// - Otherwise (including arrays): replace
pub fn json_merge<'a, I>(values: I) -> Value
where
    I: IntoIterator<Item = &'a Value>,
{
    let mut acc = Value::Object(Map::new());
    for v in values {
        merge_into(&mut acc, v.clone());
    }
    acc
}

fn merge_into(dst: &mut Value, src: Value) {
    match (dst, src) {
        (Value::Object(dst_obj), Value::Object(src_obj)) => {
            for (k, v_src) in src_obj {
                match dst_obj.get_mut(&k) {
                    Some(v_dst) => merge_into(v_dst, v_src),
                    None => {
                        dst_obj.insert(k, v_src);
                    }
                }
            }
        }
        (dst_slot, v_src) => *dst_slot = v_src,
    }
}
