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
use serde_json::map::Map as JsonMap;

const SSE_EVENT_BASE_ID: &str = "/redfish/v1/EventService/SSE";

pub(super) type EventRecordPatchFn = fn(&mut JsonMap<String, JsonValue>, usize);

pub(super) fn patch_missing_event_odata_id(mut value: JsonValue) -> JsonValue {
    let Some(payload) = value.as_object_mut() else {
        return value;
    };

    if payload.contains_key("@odata.id") {
        return value;
    }

    if let Some(event_id) = payload.get("Id").and_then(JsonValue::as_str) {
        let generated_id = format!("{SSE_EVENT_BASE_ID}#/Event{event_id}");
        payload.insert("@odata.id".to_string(), JsonValue::String(generated_id));
    }
    value
}

pub(super) fn patch_event_records(
    mut value: JsonValue,
    patches: &[EventRecordPatchFn],
) -> JsonValue {
    for_each_event_record(&mut value, |record_obj, index| {
        for patch in patches {
            patch(record_obj, index);
        }
    });
    value
}

pub(super) fn for_each_event_record<F>(value: &mut JsonValue, mut patch: F)
where
    F: FnMut(&mut JsonMap<String, JsonValue>, usize),
{
    let Some(payload) = value.as_object_mut() else {
        return;
    };

    let Some(events) = payload.get_mut("Events").and_then(JsonValue::as_array_mut) else {
        return;
    };

    for (index, record) in events.iter_mut().enumerate() {
        let Some(record_obj) = record.as_object_mut() else {
            continue;
        };
        patch(record_obj, index);
    }
}

fn is_allowed_event_type(event_type: &str) -> bool {
    serde_json::from_value::<EventType>(JsonValue::String(event_type.to_string())).is_ok()
}

pub(super) fn patch_missing_event_record_member_id(
    value: &mut JsonMap<String, JsonValue>,
    index: usize,
) {
    if value.contains_key("MemberId") {
        return;
    }

    let fallback_member_id = value
        .get("EventId")
        .and_then(JsonValue::as_str)
        .map_or_else(|| index.to_string(), ToOwned::to_owned);
    value.insert(
        "MemberId".to_string(),
        JsonValue::String(fallback_member_id),
    );
}

pub(super) fn patch_unknown_event_type_to_other(
    value: &mut JsonMap<String, JsonValue>,
    _index: usize,
) {
    let Some(JsonValue::String(event_type)) = value.get_mut("EventType") else {
        return;
    };

    if !is_allowed_event_type(event_type) {
        *event_type = "Other".to_string();
    }
}

pub(super) fn patch_missing_event_record_odata_id(
    value: &mut JsonMap<String, JsonValue>,
    _index: usize,
) {
    if value.contains_key("@odata.id") {
        return;
    }

    if let Some(member_id) = value.get("MemberId").and_then(JsonValue::as_str) {
        let generated_id = format!("{SSE_EVENT_BASE_ID}#/Events/{member_id}");
        value.insert("@odata.id".to_string(), JsonValue::String(generated_id));
    }
}

pub(super) fn patch_compact_event_timestamp_offset(
    value: &mut JsonMap<String, JsonValue>,
    _index: usize,
) {
    if let Some(JsonValue::String(timestamp)) = value.get("EventTimestamp") {
        if let Some(timestamp) = fix_timestamp_offset(timestamp) {
            value.insert("EventTimestamp".to_string(), JsonValue::String(timestamp));
        }
    }
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
    use super::EventRecordPatchFn;
    use super::fix_timestamp_offset;
    use super::patch_event_records;
    use super::patch_missing_event_record_member_id;
    use super::patch_unknown_event_type_to_other;
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
        let payload = json!({
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

        let payload = patch_event_records(
            payload,
            &[patch_unknown_event_type_to_other as EventRecordPatchFn],
        );

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

    #[test]
    fn patches_missing_member_id() {
        let payload = json!({
            "Events": [
                {
                    "EventId": "88"
                }
            ]
        });

        let payload = patch_event_records(
            payload,
            &[patch_missing_event_record_member_id as EventRecordPatchFn],
        );

        let member_id = payload
            .get("Events")
            .and_then(serde_json::Value::as_array)
            .and_then(|events| events.first())
            .and_then(|event| event.get("MemberId"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(member_id, Some("88"));
    }
}
