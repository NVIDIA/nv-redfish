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

//! Generator and scheduled-work types.
//!
//! A [`Generator`] is the leaf of the scheduling tree. It is stateful, exposes
//! readiness and an estimated next-work cost, and only manufactures executable
//! work after the scheduler selects it. Periodic flows are modeled as
//! generators, never as queues of pre-created jobs.

use core::future::Future;
use core::pin::Pin;
use core::time::Duration;
use std::time::Instant;

use crate::ids::ClassId;
use crate::ids::GeneratorId;

/// Cost units associated with a unit of work.
///
/// `CostUnits` are concrete [`u64`] newtypes. They are not generic; the runtime
/// uses them to weigh admission, fairness, and per-target/global capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CostUnits(pub u64);

impl CostUnits {
    /// Zero cost.
    pub const ZERO: Self = Self(0);

    /// Construct cost units from a raw count.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw cost value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Readiness reported by a [`Generator`] or scheduling item.
///
/// The runtime invokes [`Generator::update_ready`] before selection. A
/// generator that returns `ready: false` is not asked for work in the current
/// scan. `next_update_at` is an optional hint of when readiness should be
/// re-evaluated; `next_cost` is an optional hint of the cost of the next work
/// item (used for admission and fairness calculations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Readiness {
    /// Whether the generator currently has work that can be selected.
    pub ready: bool,
    /// Optional time when readiness should next be re-evaluated.
    pub next_update_at: Option<Instant>,
    /// Optional cost of the next work item.
    pub next_cost: Option<CostUnits>,
}

impl Readiness {
    /// Construct a "ready now" readiness with the given cost hint.
    #[must_use]
    pub const fn ready(cost: Option<CostUnits>) -> Self {
        Self {
            ready: true,
            next_update_at: None,
            next_cost: cost,
        }
    }

    /// Construct a "not ready" readiness with the given next-update hint.
    #[must_use]
    pub const fn not_ready(next_update_at: Option<Instant>) -> Self {
        Self {
            ready: false,
            next_update_at,
            next_cost: None,
        }
    }
}

/// Metadata attached to a unit of [`ScheduledWork`].
///
/// `WorkMeta` contains scheduler-relevant information about the work item. It
/// is supplied by the generator and consumed by the runtime.
#[derive(Debug, Clone)]
pub struct WorkMeta {
    /// Cost of this work item, used for admission and fairness.
    pub cost: CostUnits,
    /// Optional class identifier used for class-based scheduling.
    pub class: Option<ClassId>,
}

impl WorkMeta {
    /// Construct minimal work meta with the given cost and no class.
    #[must_use]
    pub const fn with_cost(cost: CostUnits) -> Self {
        Self { cost, class: None }
    }
}

/// Result type returned by a [`ScheduledWork`] future.
///
/// On success the work returns a vector of work events of type `Ev`. Multiple
/// events from one work item preserve order. On failure the work returns a
/// generic application or adapter error of type `Err`.
pub type ScheduledWorkResult<Ev, Err> = Result<Vec<Ev>, Err>;

/// Executable unit of work returned by a selected [`Generator`].
///
/// `ScheduledWork` carries scheduler-visible metadata together with the
/// actual work future. The future closes over whatever the generator needs.
///
/// The future is required to be `Send + 'static` so it can live in the
/// runtime's in-flight set; this matches the `Generator<Ev, Err>` storage
/// shape (`Box<dyn Generator<...> + Send>`) inside the runtime.
pub struct ScheduledWork<Ev, Err> {
    /// Scheduler-visible metadata for this work item.
    pub meta: WorkMeta,
    /// Future producing the work result.
    pub future: Pin<Box<dyn Future<Output = ScheduledWorkResult<Ev, Err>> + Send + 'static>>,
}

impl<Ev, Err> ScheduledWork<Ev, Err> {
    /// Build a [`ScheduledWork`] from work meta and a boxed future.
    #[must_use]
    pub fn new(
        meta: WorkMeta,
        future: Pin<Box<dyn Future<Output = ScheduledWorkResult<Ev, Err>> + Send + 'static>>,
    ) -> Self {
        Self { meta, future }
    }
}

/// Outcome of a single work item, reported back to the originating generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// The work future returned `Ok`.
    Succeeded,
    /// The work future returned `Err`.
    Failed,
}

/// Completion summary delivered to [`Generator::on_complete`].
///
/// Completion is reported exactly once per dispatched work item. The runtime
/// owns this struct and may extend it in later phases with latency and other
/// runtime-owned metadata.
#[derive(Debug, Clone, Copy)]
pub struct WorkCompletion {
    /// The generator that produced the work.
    pub generator_id: GeneratorId,
    /// Whether the work succeeded or failed.
    pub outcome: CompletionOutcome,
    /// Cost reported by the generator at dispatch time.
    pub cost: CostUnits,
    /// Wall-clock latency between dispatch and completion.
    pub latency: Duration,
}

/// Scheduling leaf interface implemented by application or adapter generators.
///
/// The runtime treats a generator opaquely. It only:
///
/// 1. queries readiness via [`Generator::update_ready`],
/// 2. pulls executable work via [`Generator::take_next`] for the *selected*
///    generator,
/// 3. reports completion via [`Generator::on_complete`] exactly once per
///    dispatched work item.
///
/// Removed generators are never queried again.
pub trait Generator<Ev, Err> {
    /// Refresh readiness using the supplied reference clock.
    fn update_ready(&mut self, now: Instant) -> Readiness;

    /// Produce the next executable work item, if any.
    ///
    /// Called only when the runtime selects this generator. May return `None`
    /// to indicate that no work is currently available; the runtime will then
    /// continue scanning other ready generators in the same `next` call.
    fn take_next(&mut self) -> Option<ScheduledWork<Ev, Err>>;

    /// Receive the completion summary for a previously dispatched work item.
    ///
    /// Reported exactly once per dispatched work item.
    fn on_complete(&mut self, completion: &WorkCompletion);
}
