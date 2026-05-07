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

//! Data types describing units of work as they flow between the runtime and
//! [`crate::Scheduler`] nodes.
//!
//! Includes:
//!
//! - [`CostUnits`] and [`Readiness`] used to negotiate scheduling decisions,
//! - [`WorkMeta`] attached to every [`crate::ScheduledWork`] item,
//! - [`WorkCompletion`] and [`CompletionOutcome`] reported back after dispatch,
//! - [`RoutingPath`] — the per-work breadcrumb stack that lets composable
//!   branch schedulers forward completions to the originating child without
//!   any per-branch side tables.

use core::time::Duration;
use std::time::Instant;
use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;

use crate::WorkStats;

/// Opaque identifier of a scheduler node in the dispatcher tree.
///
/// Allocated by the runtime on installation; replaced (not reused) when a
/// node is removed and another is added.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl NodeId {
    /// Construct a `NodeId` from the runtime's monotonic allocation counter.
    pub(crate) const fn from_seq(seq: u64) -> Self {
        Self(seq)
    }
}

impl Debug for NodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "NodeId({})", self.0)
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "node:{}", self.0)
    }
}

/// Cost units associated with a unit of work.
///
/// `CostUnits` are concrete [`u64`] newtypes. They are not generic; the runtime
/// uses them to weigh admission, fairness, and per-node/global capacity.
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

/// Readiness reported by a [`crate::Scheduler`] node.
///
/// The runtime invokes [`crate::Scheduler::update_ready`] before selection. A
/// node that returns `ready: false` is not asked for work in the current
/// scan. `next_update_at` is an optional hint of when readiness should be
/// re-evaluated; `next_cost` is an optional hint of the cost of the next work
/// item (used for admission and fairness calculations).
///
/// Branch nodes aggregate child readiness: ready iff any child is ready,
/// `next_update_at` is the minimum across children, `next_cost` reflects the
/// branch policy's projected next pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Readiness {
    /// Whether the node currently has work that can be selected.
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

/// Per-work routing breadcrumb used by composable branch schedulers.
///
/// Acts as a LIFO stack of child indices recorded as the work travels *up*
/// through the scheduler tree during [`crate::Scheduler::take_next`], and
/// consumed in reverse as the completion travels *down* through the tree
/// during [`crate::Scheduler::on_complete`].
///
/// Protocol:
///
/// 1. A leaf creates the work with an empty path and never inspects it again.
/// 2. Each branch on `take_next`, after it has selected child `i` and
///    received that child's `ScheduledWork`, calls
///    [`RoutingPath::push`] with `i` on `work.meta.routing` before returning
///    upward.
/// 3. The runtime carries the path verbatim through dispatch and copies it
///    onto the [`WorkCompletion`].
/// 4. Each branch on `on_complete` pops its own tag with [`RoutingPath::pop`]
///    and forwards the same `&mut WorkCompletion` to that child's
///    `on_complete`. The leaf at the bottom finds the path empty.
///
/// The runtime itself never inspects the path — it is purely a node-to-node
/// channel.
///
/// Internally backed by a [`Vec`] for the scaffold; later phases may swap to
/// a small inline buffer (`smallvec`-style) without changing the API.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutingPath {
    inner: Vec<u32>,
}

impl RoutingPath {
    /// Construct an empty routing path. Always the starting state at a leaf.
    #[must_use]
    pub const fn empty() -> Self {
        Self { inner: Vec::new() }
    }

    /// Push a child index onto the path. Called by a branch after selecting
    /// a child during [`crate::Scheduler::take_next`].
    pub fn push(&mut self, child_idx: u32) {
        self.inner.push(child_idx);
    }

    /// Pop the most recent child index. Called by a branch at the start of
    /// [`crate::Scheduler::on_complete`] to recover its own selection.
    #[must_use]
    pub fn pop(&mut self) -> Option<u32> {
        self.inner.pop()
    }

    /// Current depth of the path (number of branches that have stamped it).
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.inner.len()
    }

    /// `true` when the path is empty (a leaf has not been reached yet, or the
    /// completion has been fully forwarded).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Metadata attached to a unit of [`crate::ScheduledWork`].
///
/// `WorkMeta` is the scheduler-visible projection of a work item. It carries
/// the cost and the routing breadcrumb stack; the actual work future lives
/// alongside it inside [`crate::ScheduledWork`].
#[derive(Debug, Clone)]
pub struct WorkMeta {
    /// Cost of this work item, used for admission and fairness.
    pub cost: CostUnits,
    /// Routing breadcrumb stack populated by branch schedulers as the work
    /// travels up to the runtime. See [`RoutingPath`] for the protocol.
    pub routing: RoutingPath,
}

impl WorkMeta {
    /// Construct minimal work meta with the given cost and an empty routing
    /// path. Leaves use this; branches mutate `routing` after the fact.
    #[must_use]
    pub const fn with_cost(cost: CostUnits) -> Self {
        Self {
            cost,
            routing: RoutingPath::empty(),
        }
    }
}

/// Outcome of a single work item, reported back through the scheduler tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// The work future returned `Ok`.
    Succeeded,
    /// The work future returned `Err`.
    Failed,
}

/// Completion summary delivered to [`crate::Scheduler::on_complete`].
///
/// Reported exactly once per dispatched work item. The runtime owns this
/// struct; branches mutate `routing` in place as they pop their tag and
/// forward to children, so the signature uses `&mut WorkCompletion`.
///
/// `node_id` is the id of the *root* node the work was dispatched under.
/// Internal leaf identity (if any) is recovered by following `routing`.
#[derive(Debug, Clone)]
pub struct WorkCompletion {
    /// The root node the work was dispatched under.
    pub node_id: NodeId,
    /// Whether the work succeeded or failed.
    pub outcome: CompletionOutcome,
    /// Cost reported by the originating leaf at dispatch time.
    pub cost: CostUnits,
    /// Wall-clock latency between dispatch and completion.
    pub latency: Duration,
    /// Routing breadcrumb stack copied from the dispatched work. Branches
    /// pop their own tag and forward to the indicated child.
    pub routing: RoutingPath,
}

/// Successful work output: a vector of events with runtime-owned stats.
///
/// Multiple events from one work item preserve order. An empty event vector
/// is allowed and still constitutes a successful output.
pub struct WorkSuccess<Ev> {
    /// Events produced by the work item, in order.
    pub events: Vec<Ev>,
    /// Runtime-owned statistics for this work item.
    pub stats: WorkStats,
    /// The root node the work was dispatched under.
    pub node_id: NodeId,
}

/// Failed work output: the error value plus runtime-owned stats.
pub struct WorkError<Err> {
    /// The error returned by the work future.
    pub error: Err,
    /// Runtime-owned statistics for this work item.
    pub stats: WorkStats,
    /// The root node the work was dispatched under.
    pub node_id: NodeId,
}

/// Result alias used inside [`RuntimeOutput::Work`].
pub type WorkResult<Ev, Err> = Result<WorkSuccess<Ev>, WorkError<Err>>;
