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

//! Patches for EventService SSE payloads.
//!
//! OData ABNF reference:
//! <https://docs.oasis-open.org/odata/odata/v4.01/os/abnf/odata-abnf-construction-rules.txt>

use crate::schema::redfish::event::EventType;
use serde_json::Value as JsonValue;

const SSE_EVENT_BASE_ID: &str = "/redfish/v1/EventService/SSE";

pub(super) fn normalize_event_payload(value: &mut JsonValue) {
    let Some(payload) = value.as_object_mut() else {
        return;
    };

    if !payload.contains_key("@odata.id") {
        if let Some(event_id) = payload.get("Id").and_then(JsonValue::as_str) {
            let generated_id = format!("{SSE_EVENT_BASE_ID}#/Event{event_id}");
            payload.insert("@odata.id".to_string(), JsonValue::String(generated_id));
        }
    }

    if let Some(events) = payload.get_mut("Events").and_then(JsonValue::as_array_mut) {
        for (index, record) in events.iter_mut().enumerate() {
            let Some(record_obj) = record.as_object_mut() else {
                continue;
            };

            if !record_obj.contains_key("MemberId") {
                let fallback_member_id = record_obj
                    .get("EventId")
                    .and_then(JsonValue::as_str)
                    .map_or_else(|| index.to_string(), ToOwned::to_owned);
                record_obj.insert(
                    "MemberId".to_string(),
                    JsonValue::String(fallback_member_id),
                );
            }

            if let Some(JsonValue::String(event_type)) = record_obj.get_mut("EventType") {
                if !is_allowed_event_type(event_type) {
                    *event_type = "Other".to_string();
                }
            }

            if !record_obj.contains_key("@odata.id") {
                if let Some(member_id) = record_obj.get("MemberId").and_then(JsonValue::as_str) {
                    let generated_id = format!("{SSE_EVENT_BASE_ID}#/Events/{member_id}");
                    record_obj.insert("@odata.id".to_string(), JsonValue::String(generated_id));
                }
            }

            if let Some(JsonValue::String(timestamp)) = record_obj.get("EventTimestamp") {
                if let Some(timestamp) = fix_timestamp_offset(timestamp) {
                    record_obj.insert("EventTimestamp".to_string(), JsonValue::String(timestamp));
                }
            }
        }
    }
}

fn is_allowed_event_type(event_type: &str) -> bool {
    serde_json::from_value::<EventType>(JsonValue::String(event_type.to_string())).is_ok()
}

fn fix_timestamp_offset(input: &str) -> Option<String> {
    let sign_index = input.len().checked_sub(5)?;
    let suffix = input.get(sign_index..)?;
    let mut chars = suffix.chars();
    let sign = chars.next()?;
    if sign != '+' && sign != '-' {
        return None;
    }

    let prefix = input.get(..(sign_index + 3))?;
    let minutes = input.get((sign_index + 3)..)?;
    Some(format!("{prefix}:{minutes}"))
}

#[cfg(test)]
mod tests {
    use super::fix_timestamp_offset;
    use super::normalize_event_payload;
    use serde_json::json;

    #[test]
    fn normalizes_compact_offset() {
        let fixed = fix_timestamp_offset("2017-11-23T17:17:42-0600");
        assert_eq!(fixed, Some("2017-11-23T17:17:42-06:00".to_string()));
    }

    #[test]
    fn keeps_rfc3339_offset_unchanged() {
        assert_eq!(fix_timestamp_offset("2017-11-23T17:17:42-06:00"), None);
    }

    #[test]
    fn replaces_unknown_event_type_with_other() {
        let mut payload = json!({
            "Events": [
                {
                    "EventType": "Event"
                },
                {
                    "EventType": "FooBar"
                },
                {
                    "EventType": "Alert"
                }
            ]
        });

        normalize_event_payload(&mut payload);

        let events = payload
            .get("Events")
            .and_then(serde_json::Value::as_array)
            .expect("events array");
        assert_eq!(
            events[0]
                .get("EventType")
                .and_then(serde_json::Value::as_str),
            Some("Other")
        );
        assert_eq!(
            events[1]
                .get("EventType")
                .and_then(serde_json::Value::as_str),
            Some("Other")
        );
        assert_eq!(
            events[2]
                .get("EventType")
                .and_then(serde_json::Value::as_str),
            Some("Alert")
        );
    }
}
