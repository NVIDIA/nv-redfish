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
    }
}

#[cfg(feature = "runtime-events")]
pub use with_events::RuntimeEvent;
