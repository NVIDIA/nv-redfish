// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::de::Error as _;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde_json::Value;

use crate::ResourceSnapshot;

/// Snapshot for an untyped Redfish resource.
pub type RawSnapshot = ResourceSnapshot<RawResource>;

/// Untyped Redfish resource used as an OEM and unknown-resource escape hatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RawResource {
    id: ODataId,
    etag: Option<ODataETag>,
    value: Value,
}

impl RawResource {
    /// Creates a raw resource from its identity metadata and JSON value.
    #[must_use]
    pub const fn new(id: ODataId, etag: Option<ODataETag>, value: Value) -> Self {
        Self { id, etag, value }
    }

    /// Returns the complete JSON payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

impl EntityTypeRef for RawResource {
    fn odata_id(&self) -> &ODataId {
        &self.id
    }

    fn etag(&self) -> Option<&ODataETag> {
        self.etag.as_ref()
    }
}

impl<'de> Deserialize<'de> for RawResource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("raw Redfish resource must be a JSON object"))?;
        let id = object
            .get("@odata.id")
            .and_then(Value::as_str)
            .map(|id| ODataId::from(id.to_owned()))
            .ok_or_else(|| D::Error::custom("raw Redfish resource is missing @odata.id"))?;
        let etag = object
            .get("@odata.etag")
            .and_then(Value::as_str)
            .map(|etag| ODataETag::from(etag.to_owned()));
        Ok(Self { id, etag, value })
    }
}

impl Display for RawResource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "{}", self.id)
    }
}
