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

//! Statistics snapshot types.
//!
//! Snapshots are point-in-time views and do not require atomic reads across
//! multiple sub-snapshots. The dispatcher reports per-node and runtime-wide
//! counters; per-node-type policy details (DRR weights, token-bucket levels,
//! etc.) live with each branch implementation and are inspected through that
//! impl's own API.

use core::time::Duration;

use crate::work::NodeId;

/// Per-work statistics owned by the runtime.
///
/// The runtime attaches `WorkStats` to every successful and failed work
/// output. Nodes do not fabricate runtime statistics themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkStats {
    /// Wall-clock latency between dispatch and completion.
    pub latency: Duration,
}

/// Per-node statistics snapshot.
///
/// Reported uniformly for every node in the tree (leaves and branches alike).
/// For branch nodes, the counters aggregate dispatches and completions that
/// flowed through the node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeStats {
    /// Number of work items dispatched through this node.
    pub dispatched: u64,
    /// Number of successfully completed work items.
    pub succeeded: u64,
    /// Number of failed work items.
    pub failed: u64,
    /// Number of work items currently in flight.
    pub in_flight: u64,
    /// Lag behind the requested rate (leaf-only; branches report 0).
    pub missed_intervals: u64,
    /// Most recently observed actual interval between dispatches
    /// (leaf-only; branches report `None`).
    pub actual_interval: Option<Duration>,
}

/// Top-level runtime statistics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStats {
    /// Number of registered nodes (root + every child in the tree).
    pub nodes: u64,
    /// Number of work items currently in flight runtime-wide.
    pub in_flight: u64,
    /// Number of work items dispatched runtime-wide.
    pub dispatched: u64,
    /// Output queue stats.
    pub output_queue: OutputQueueStats,
    /// Per-node snapshots in tree-walk order.
    pub per_node: Vec<(NodeId, NodeStats)>,
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
