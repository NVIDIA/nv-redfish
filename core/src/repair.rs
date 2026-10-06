// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Repairing documents before they are deserialized.
//!
//! A [`Repair`] rewrites a document a device sent so it matches the schema
//! type it is read as. A transport that deserializes documents itself can
//! apply it in the same pass, as [`crate::Bmc::get_repaired`] lets it; the
//! default reads the document as [`crate::Raw`], repairs it, and
//! deserializes it once more.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::ActionError;
use crate::Raw;

/// A rewrite of documents before they are deserialized.
pub trait Repair: Send + Sync {
    /// Rewrites `document` in place.
    fn repair(&self, document: &mut Value);

    /// Identifies the rewrite: two repairs with the same generation rewrite
    /// every document the same way. A transport that caches repaired
    /// results keys them by it, so a result repaired under another
    /// generation is not served.
    fn generation(&self) -> u64;
}

/// A repaired document that still did not deserialize, with the path to
/// the property that failed.
pub type DecodeError = serde_path_to_error::Error<serde_json::Error>;

/// What a repaired read can fail with.
#[derive(Debug)]
pub enum RepairError<E> {
    /// The transport failed. A transport that repairs as it deserializes
    /// reports a document that does not deserialize here, as it would
    /// for an unrepaired read.
    Transport(E),
    /// The document, repaired, did not deserialize into the type asked
    /// for.
    Decode(DecodeError),
}

impl<E: fmt::Display> fmt::Display for RepairError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "BMC error: {error}"),
            Self::Decode(error) => write!(f, "repaired document did not deserialize: {error}"),
        }
    }
}

impl<E: StdError + 'static> StdError for RepairError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Decode(error) => Some(error),
        }
    }
}

impl<E: ActionError> ActionError for RepairError<E> {
    fn not_supported() -> Self {
        Self::Transport(E::not_supported())
    }
}

/// Repairs a raw read's document and deserializes it into `T`: the path a
/// transport without its own takes.
pub(crate) fn decode_raw<T, E>(
    raw: Result<Arc<Raw>, E>,
    repair: &dyn Repair,
) -> Result<Arc<T>, RepairError<E>>
where
    T: for<'de> Deserialize<'de>,
{
    let raw = raw.map_err(RepairError::Transport)?;
    // A transport that does not cache hands over the only reference.
    let mut document = Arc::try_unwrap(raw).map_or_else(|raw| raw.value().clone(), Raw::into_value);
    repair.repair(&mut document);
    serde_path_to_error::deserialize(document)
        .map(Arc::new)
        .map_err(RepairError::Decode)
}
