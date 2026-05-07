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
    use crate::ids::GeneratorId;
    use crate::ids::TargetId;

    /// Out-of-band scheduler, executor, and queue events emitted by the
    /// runtime when the `runtime-events` feature is enabled.
    ///
    /// These events are interleaved with work outputs in [`crate::Runtime::next`]
    /// preserving causal ordering. They never carry user work payloads.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum RuntimeEvent {
        /// A generator is lagging behind its requested rate.
        GeneratorLagging {
            /// The lagging generator.
            generator_id: GeneratorId,
        },
        /// A previously-lagging generator has caught up.
        GeneratorRecovered {
            /// The recovered generator.
            generator_id: GeneratorId,
        },
        /// A generator is being starved by other flows.
        GeneratorStarved {
            /// The starved generator.
            generator_id: GeneratorId,
        },
        /// A target is being throttled by per-target capacity.
        TargetThrottled {
            /// The throttled target.
            target_id: TargetId,
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
            /// The generator that produced the work.
            generator_id: GeneratorId,
        },
        /// Work completed successfully. Brackets the corresponding
        /// `RuntimeOutput::Work(Ok(_))` together with [`RuntimeEvent::WorkStarted`].
        WorkCompleted {
            /// The generator that produced the work.
            generator_id: GeneratorId,
        },
        /// Work failed. Brackets the corresponding `RuntimeOutput::Work(Err(_))`
        /// together with [`RuntimeEvent::WorkStarted`].
        WorkFailed {
            /// The generator that produced the work.
            generator_id: GeneratorId,
        },
        /// A point-in-time snapshot of scheduler statistics. Phase 0 reserves
        /// the variant; concrete payload fields are added in later phases.
        SchedulerStatsSnapshot,
    }
}

#[cfg(feature = "runtime-events")]
pub use with_events::RuntimeEvent;
