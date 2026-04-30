// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use tokio::time::Instant;

/// Typed resource snapshot stored by the scraper.
///
/// Snapshots are immutable and cheap to clone. Direct refresh returns fresh
/// snapshots in this phase.
#[derive(Debug)]
pub struct ResourceSnapshot<T> {
    /// Resource `@odata.id`.
    pub id: ODataId,
    /// Typed Redfish resource value.
    pub value: Arc<T>,
    /// Optional resource `@odata.etag`.
    pub etag: Option<ODataETag>,
    /// Time when the value was fetched from the BMC.
    pub fetched_at: SystemTime,
    /// Freshness state reported to the caller.
    pub staleness: Staleness,
    pub(crate) observed_at: Instant,
}

impl<T> Clone for ResourceSnapshot<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            value: Arc::clone(&self.value),
            etag: self.etag.clone(),
            fetched_at: self.fetched_at,
            staleness: self.staleness,
            observed_at: self.observed_at,
        }
    }
}

impl<T> ResourceSnapshot<T> {
    pub(crate) fn new_fresh(id: ODataId, value: Arc<T>, etag: Option<ODataETag>) -> Self {
        Self {
            id,
            value,
            etag,
            fetched_at: SystemTime::now(),
            staleness: Staleness::Fresh,
            observed_at: Instant::now(),
        }
    }

    pub(crate) fn with_desired_freshness(mut self, desired: Option<Duration>) -> Self {
        if let Some(desired) = desired {
            let age = self.observed_at.elapsed();
            self.staleness = if age > desired {
                Staleness::Stale {
                    age,
                    desired: Some(desired),
                }
            } else {
                Staleness::Fresh
            };
        }
        self
    }

    pub(crate) fn is_stale_for(&self, desired: Duration) -> bool {
        self.observed_at.elapsed() >= desired
    }

    pub(crate) fn age(&self) -> Duration {
        self.observed_at.elapsed()
    }
}

/// Freshness state for a resource snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Staleness {
    /// The snapshot satisfies the requested freshness.
    Fresh,
    /// The snapshot is older than requested, but is still returned honestly.
    Stale {
        /// Current snapshot age.
        age: Duration,
        /// Desired freshness, when known.
        desired: Option<Duration>,
    },
}
