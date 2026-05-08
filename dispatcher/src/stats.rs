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

//! Point-in-time stats snapshots.
//!
//! Only runtime-wide aggregates live here; per-node policy details (DRR
//! weights, token-bucket levels, leaf lag, …) are a scheduler concern,
//! reachable via [`crate::RuntimeHandle::with_root`].

/// Runtime-wide statistics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStats {
    /// Items currently in flight.
    pub in_flight: u64,
    /// Items dispatched since startup.
    pub dispatched: u64,
    /// Output queue stats.
    pub output_queue: OutputQueueStats,
}

/// Output queue pressure and drop accounting. Bounded queues report
/// pressure as length + drops, never as unbounded growth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutputQueueStats {
    /// Outputs currently queued.
    pub queued: usize,
    /// Configured upper bound, if any.
    pub capacity: Option<usize>,
    /// Outputs dropped or rejected under pressure.
    pub dropped: u64,
}
