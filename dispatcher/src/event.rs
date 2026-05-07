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

//! Runtime event types.
//!
//! Runtime events describe out-of-band scheduler, executor, and queue facts
//! such as lag, starvation, throttling, and queue pressure. They are
//! compile-time feature gated. When the `runtime-events` feature is disabled,
//! [`RuntimeEventType`] is [`Infallible`] and emission paths are not compiled.

#[cfg(not(feature = "runtime-events"))]
use core::convert::Infallible;

/// Concrete payload carried by [`crate::RuntimeOutput::Runtime`].
///
/// When the `runtime-events` feature is enabled, this aliases the
/// [`RuntimeEvent`] enum. Otherwise it aliases [`Infallible`], making the
/// `Runtime` variant uninhabited and therefore impossible to construct from
/// outside the crate.
#[cfg(feature = "runtime-events")]
pub type RuntimeEventType = RuntimeEvent;

/// Concrete payload carried by [`crate::RuntimeOutput::Runtime`].
#[cfg(not(feature = "runtime-events"))]
pub type RuntimeEventType = Infallible;

#[cfg(feature = "runtime-events")]
mod with_events {
    use crate::ids::NodeId;

    /// Out-of-band scheduler, executor, and queue events emitted by the
    /// runtime when the `runtime-events` feature is enabled.
    ///
    /// These events are interleaved with work outputs in
    /// [`crate::Runtime::next`] preserving causal ordering. They never carry
    /// user work payloads.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum RuntimeEvent {
        /// A node is lagging behind its requested rate.
        NodeLagging {
            /// The lagging node.
            node_id: NodeId,
        },
        /// A previously-lagging node has caught up.
        NodeRecovered {
            /// The recovered node.
            node_id: NodeId,
        },
        /// A node is being starved by other flows.
        NodeStarved {
            /// The starved node.
            node_id: NodeId,
        },
        /// The runtime is being throttled by global capacity.
        GlobalThrottled,
        /// The output queue is under pressure.
        EventQueuePressure {
            /// Current queue depth.
            queued: usize,
        },
        /// Work was dispatched and started executing.
        WorkStarted {
            /// The root node the work was dispatched under.
            node_id: NodeId,
        },
        /// Work completed successfully. Brackets the corresponding
        /// `RuntimeOutput::Work(Ok(_))` together with [`RuntimeEvent::WorkStarted`].
        WorkCompleted {
            /// The root node the work was dispatched under.
            node_id: NodeId,
        },
        /// Work failed. Brackets the corresponding `RuntimeOutput::Work(Err(_))`
        /// together with [`RuntimeEvent::WorkStarted`].
        WorkFailed {
            /// The root node the work was dispatched under.
            node_id: NodeId,
        },
        /// A point-in-time snapshot of scheduler statistics. Reserved
        /// variant; concrete payload fields are added in later phases.
        SchedulerStatsSnapshot,
    }
}

#[cfg(feature = "runtime-events")]
pub use with_events::RuntimeEvent;
