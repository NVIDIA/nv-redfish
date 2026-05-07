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

//! The unified [`Scheduler`] trait — every node in the dispatcher's scheduling
//! tree implements it.
//!
//! The trait subsumes both *leaves* (nodes that produce work directly, the
//! role today's `Generator` plays) and *branches* (nodes that compose
//! children using a scheduling policy: weighted DRR, round-robin, priority,
//! token-bucket admission, etc.). The runtime treats every node identically:
//! it queries readiness, asks for one work item, and forwards completions.
//!
//! Branch nodes implement scheduling policy by:
//!
//! - aggregating child readiness in [`Scheduler::update_ready`],
//! - selecting a child in [`Scheduler::take_next`], pushing the child index
//!   onto the returned work's [`crate::RoutingPath`] before returning upward,
//! - popping the routing tag in [`Scheduler::on_complete`] and forwarding to
//!   the originating child.
//!
//! Leaf nodes ignore the routing path; they account locally and produce work
//! futures that close over whatever the application needs.

use core::future::Future;
use core::pin::Pin;
use std::time::Instant;

use crate::work::Readiness;
use crate::work::WorkCompletion;
use crate::work::WorkMeta;

/// Result type returned by a [`ScheduledWork`] future.
///
/// On success the work returns a vector of work events of type `Ev`. Multiple
/// events from one work item preserve order. On failure the work returns a
/// generic application or adapter error of type `Err`.
pub type ScheduledWorkResult<Ev, Err> = Result<Vec<Ev>, Err>;

/// Executable unit of work returned by a selected [`Scheduler`] node.
///
/// `ScheduledWork` carries scheduler-visible metadata together with the
/// actual work future. The future closes over whatever the leaf needs.
///
/// The future is required to be `Send + 'static` so it can live in the
/// runtime's in-flight set; this matches the `Scheduler<Ev, Err>` storage
/// shape (`Box<dyn Scheduler<...> + Send>`) inside the runtime.
pub struct ScheduledWork<Ev, Err> {
    /// Scheduler-visible metadata for this work item, including its
    /// [`crate::RoutingPath`].
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

/// Composable scheduler node interface.
///
/// The runtime drives the *root* node by:
///
/// 1. querying readiness via [`Scheduler::update_ready`] before selection,
/// 2. pulling executable work via [`Scheduler::take_next`] when admission
///    permits,
/// 3. reporting completion via [`Scheduler::on_complete`] exactly once per
///    dispatched work item.
///
/// Branch implementations recurse: their `take_next` calls a chosen child's
/// `take_next`, their `on_complete` pops a routing tag and forwards to the
/// indicated child. Leaf implementations produce work and account locally.
///
/// Removed nodes are never queried again.
pub trait Scheduler<Ev, Err> {
    /// Refresh readiness using the supplied reference clock.
    ///
    /// Branches should aggregate across children (any-ready -> ready,
    /// `next_update_at` = min over children, `next_cost` per branch policy).
    fn update_ready(&mut self, now: Instant) -> Readiness;

    /// Produce the next executable work item, if any.
    ///
    /// Called only when the runtime selects this node (or, recursively, when
    /// a parent branch selects this node). May return `None` to indicate that
    /// no work is currently available; the runtime/parent will then continue
    /// scanning other ready children.
    ///
    /// Branch contract: after a successful child selection, push the chosen
    /// child index onto `work.meta.routing` before returning.
    fn take_next(&mut self) -> Option<ScheduledWork<Ev, Err>>;

    /// Receive the completion summary for a previously dispatched work item.
    ///
    /// Reported exactly once per dispatched work item. The runtime delivers
    /// completions to the *root* node; branches pop their own routing tag
    /// from `completion.routing` and forward to the originating child.
    fn on_complete(&mut self, completion: &mut WorkCompletion);
}
