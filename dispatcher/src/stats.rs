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
//! Phase 0 fixes the snapshot shapes; later phases populate them with real
//! counters. Snapshots are point-in-time views and do not require atomic reads
//! across multiple sub-snapshots.

use core::time::Duration;

use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::output::OutputQueueStats;

/// Per-work statistics owned by the runtime.
///
/// The runtime attaches `WorkStats` to every successful and failed work
/// output. Generators do not fabricate runtime statistics themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkStats {
    /// Wall-clock latency between dispatch and completion.
    pub latency: Duration,
}

/// Per-generator statistics snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeneratorStats {
    /// Number of work items dispatched by this generator.
    pub dispatched: u64,
    /// Number of successfully completed work items.
    pub succeeded: u64,
    /// Number of failed work items.
    pub failed: u64,
    /// Number of work items currently in flight.
    pub in_flight: u64,
    /// Lag behind the requested rate, expressed as missed periods so far.
    pub missed_intervals: u64,
    /// Most recently observed actual interval between dispatches.
    pub actual_interval: Option<Duration>,
}

/// Per-class statistics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassStats {
    /// The class identifier.
    pub class: Option<ClassId>,
    /// Number of work items dispatched in this class.
    pub dispatched: u64,
    /// Number of work items currently in flight in this class.
    pub in_flight: u64,
}

/// Per-target statistics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetStats {
    /// The target identifier.
    pub target: Option<TargetId>,
    /// Number of attached generators.
    pub generators: u64,
    /// Number of work items currently in flight against this target.
    pub in_flight: u64,
    /// Number of work items dispatched against this target.
    pub dispatched: u64,
    /// Stats for the generators attached to this target.
    pub per_generator: Vec<(GeneratorId, GeneratorStats)>,
}

/// Top-level runtime statistics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStats {
    /// Number of registered targets.
    pub targets: u64,
    /// Number of registered generators across all targets.
    pub generators: u64,
    /// Number of work items currently in flight runtime-wide.
    pub in_flight: u64,
    /// Number of work items dispatched runtime-wide.
    pub dispatched: u64,
    /// Output queue stats.
    pub output_queue: OutputQueueStats,
    /// Per-target snapshots.
    pub per_target: Vec<TargetStats>,
}
