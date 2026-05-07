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

//! Runtime output and ordered work-result types.
//!
//! Output is delivered through a single ordered stream consumed via
//! [`crate::Runtime::next`]. The combined stream preserves causal ordering
//! across successful work, failed work, and runtime events.
//!
//! None of the wrappers here derive `Clone`, `Debug`, `Eq`, or `PartialEq`,
//! because that would impose those bounds on the user-supplied event type
//! `Ev` and error type `Err`. Manual implementations are provided where the
//! runtime needs them and they do not require bounds on the generic
//! parameters.

use crate::event::RuntimeEventType;
use crate::ids::GeneratorId;
use crate::stats::WorkStats;

/// Successful work output: a vector of events with runtime-owned stats.
///
/// Multiple events from one work item preserve order. An empty event vector
/// is allowed and still constitutes a successful output.
pub struct WorkSuccess<Ev> {
    /// Events produced by the work item, in order.
    pub events: Vec<Ev>,
    /// Runtime-owned statistics for this work item.
    pub stats: WorkStats,
    /// The generator that produced the work.
    pub generator_id: GeneratorId,
}

/// Failed work output: the error value plus runtime-owned stats.
pub struct WorkError<Err> {
    /// The error returned by the work future.
    pub error: Err,
    /// Runtime-owned statistics for this work item.
    pub stats: WorkStats,
    /// The generator that produced the work.
    pub generator_id: GeneratorId,
}

/// Result alias used inside [`RuntimeOutput::Work`].
pub type WorkResult<Ev, Err> = Result<WorkSuccess<Ev>, WorkError<Err>>;

/// Single ordered output value emitted by the runtime.
///
/// `R` defaults to [`crate::RuntimeEventType`] which is
/// [`core::convert::Infallible`] when the `runtime-events` feature is
/// disabled, making `RuntimeOutput::Runtime(_)` impossible to construct.
pub enum RuntimeOutput<Ev, Err, R = RuntimeEventType> {
    /// Application or adapter work output.
    Work(WorkResult<Ev, Err>),
    /// Out-of-band runtime event. Only constructible when `runtime-events`
    /// is enabled (otherwise `R = Infallible`).
    Runtime(R),
    /// Sticky terminal output emitted after graceful shutdown drains in-flight
    /// work and prior queued output. Subsequent `next()` calls return this
    /// variant immediately.
    Shutdown,
}

/// Output queue pressure and drop accounting.
///
/// Bounded output queues report pressure through a bounded length plus a
/// dropped-or-rejected count, never through unbounded queue growth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutputQueueStats {
    /// Current number of queued outputs awaiting consumption.
    pub queued: usize,
    /// Configured upper bound on the queue, if any.
    pub capacity: Option<usize>,
    /// Number of outputs dropped or rejected due to capacity pressure.
    pub dropped: u64,
}
