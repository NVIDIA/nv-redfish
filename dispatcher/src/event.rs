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

//! Out-of-band runtime events (throttling, queue pressure, …).
//!
//! Feature-gated by `runtime-events`. When the feature is off,
//! [`RuntimeEventType`] is [`core::convert::Infallible`] and emission
//! paths are not compiled.

#[cfg(not(feature = "runtime-events"))]
use core::convert::Infallible;
use std::sync::Arc;

/// Stable runtime-assigned identity for an externally-fed queue.
///
/// IDs are unique within one runtime and become available when the queue
/// scheduler is attached to that runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueueId(u64);

impl QueueId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Raw runtime-local queue identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Signal sent by an externally-fed queue to its runtime.
///
/// [`QueueEvent::WakeUp`] is control-plane only and never appears in
/// [`crate::RuntimeOutput`]. `Drained` becomes a
/// `RuntimeEvent::QueueDrained` output when `runtime-events` is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueEvent {
    /// Queue readiness may have changed.
    WakeUp,
    /// A closed queue has no queued or in-flight work remaining.
    Drained {
        /// Stable identity assigned when the queue attached to runtime.
        queue_id: QueueId,
    },
}

type QueueEventHandler = dyn Fn(QueueEvent) + Send + Sync + 'static;
type QueueIdAllocator = dyn Fn() -> QueueId + Send + Sync + 'static;

/// Restricted capability used by queue schedulers to signal their runtime.
///
/// It accepts only [`QueueEvent`], preventing queue implementations from
/// fabricating unrelated runtime events. The sink owns runtime wake-up;
/// schedulers never receive a raw [`std::task::Waker`].
#[derive(Clone)]
pub struct QueueEventSink {
    handler: Arc<QueueEventHandler>,
    allocator: Arc<QueueIdAllocator>,
    runtime_identity: Arc<()>,
}

impl QueueEventSink {
    pub(crate) fn new(
        handler: impl Fn(QueueEvent) + Send + Sync + 'static,
        allocator: impl Fn() -> QueueId + Send + Sync + 'static,
        runtime_identity: Arc<()>,
    ) -> Self {
        Self {
            handler: Arc::new(handler),
            allocator: Arc::new(allocator),
            runtime_identity,
        }
    }

    /// Allocate a stable identifier local to this runtime.
    ///
    /// Custom externally-fed queue schedulers use this when the sink is
    /// first registered. Clones of a sink share one allocation domain.
    #[must_use]
    pub fn allocate_queue_id(&self) -> QueueId {
        (self.allocator)()
    }

    /// Push a queue control or lifecycle event.
    pub fn push(&self, event: QueueEvent) {
        (self.handler)(event);
    }

    pub(crate) fn belongs_to_same_runtime(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.runtime_identity, &other.runtime_identity)
    }
}

/// Concrete payload carried by [`crate::RuntimeOutput::Runtime`].
#[cfg(feature = "runtime-events")]
pub type RuntimeEventType = RuntimeEvent;

/// Concrete payload carried by [`crate::RuntimeOutput::Runtime`].
#[cfg(not(feature = "runtime-events"))]
pub type RuntimeEventType = Infallible;

#[cfg(feature = "runtime-events")]
mod with_events {
    /// Out-of-band runtime events emitted when `runtime-events` is on.
    /// Interleaved with work outputs in [`crate::Runtime::next`] in causal
    /// order; they never carry user payloads.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum RuntimeEvent {
        /// Throttled by the global in-flight cap.
        GlobalThrottled,
        /// Output queue is under pressure.
        EventQueuePressure {
            /// Current queue depth.
            queued: usize,
        },
        /// A payload was dispatched.
        WorkStarted,
        /// A payload completed successfully; brackets a `Work { Ok, .. }`
        /// output together with [`RuntimeEvent::WorkStarted`].
        WorkCompleted,
        /// A payload failed; brackets a `Work { Err, .. }` output together
        /// with [`RuntimeEvent::WorkStarted`].
        WorkFailed,
        /// Reserved snapshot variant; payload fields land later.
        SchedulerStatsSnapshot,
        /// A closed externally-fed queue has fully drained.
        QueueDrained {
            /// Stable runtime-assigned queue identity.
            queue_id: super::QueueId,
        },
    }
}

#[cfg(feature = "runtime-events")]
pub use with_events::RuntimeEvent;
