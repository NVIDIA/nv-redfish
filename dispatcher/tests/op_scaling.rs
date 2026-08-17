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

//! Hardware-independent cost measurements.

use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use std::sync::Arc;

use nv_redfish_dispatcher::{CostUnits, FixedCost, ManualClock, PeriodicLeaf, RoundRobin};
use nv_redfish_dispatcher_sim::{
    ample_bucket, expected_dispatches, simulate, source, Counted, OpCounts, Work,
};

/// Root passes beyond one-per-dispatch over a run: the initial pass,
/// one per wave boundary ending in a `SleepUntil`, and the shutdown
/// drain. Independent of fleet size.
const EXTRA_PASSES: u64 = 12;

struct Ops {
    update_ready: u64,
    take_next: u64,
    dispatches: u64,
}

fn ops(counts: &OpCounts, dispatches: usize) -> Ops {
    Ops {
        update_ready: counts.update_ready.load(Ordering::Relaxed),
        take_next: counts.take_next.load(Ordering::Relaxed),
        dispatches: dispatches as u64,
    }
}

/// Uniform fleet, one virtual minute. All sources share one epoch, so
/// readiness arrives in synchronized waves: whenever the root rotates,
/// every child is ready.
async fn measure_dense(n: u32) -> Ops {
    let counts = Arc::new(OpCounts::default());
    let no_fail = Arc::new(AtomicBool::new(false));
    let clock = ManualClock::new();
    let now = clock.now();
    let mut root = RoundRobin::new();
    for idx in 0..n {
        root.add_child(Counted::new(
            counts.clone(),
            source(now, idx, ample_bucket(), no_fail.clone()),
        ));
    }

    let log = simulate(clock, root, Duration::from_secs(60), Vec::new()).await;
    ops(&counts, log.len())
}

/// One single-fire leaf per source, staggered in *reverse* rotation
/// order: exactly one source is due per wave and the cursor is
/// maximally misaligned.
async fn measure_sparse(n: u32) -> Ops {
    let counts = Arc::new(OpCounts::default());
    let clock = ManualClock::new();
    let now = clock.now();
    let mut root = RoundRobin::new();
    for idx in 0..n {
        let first_due = now + Duration::from_secs(u64::from(n - 1 - idx));
        let leaf =
            PeriodicLeaf::starting_at(now, first_due, Duration::from_secs(3600), move || {
                Box::pin(async move { Ok(vec![(idx, 0_u8)]) }) as Work
            });
        root.add_child(Counted::new(
            counts.clone(),
            FixedCost::new(CostUnits::new(1), leaf),
        ));
    }

    let log = simulate(clock, root, Duration::from_secs(u64::from(n)), Vec::new()).await;
    ops(&counts, log.len())
}

/// Dense regime: one readiness scan of every child per admitted item,
/// and the first probe always hits.
#[tokio::test]
async fn dense_readiness_cost_matches_the_exact_scan_model() {
    let window = Duration::from_secs(60);
    for n in [100_u32, 200, 400] {
        let m = measure_dense(n).await;
        let n = u64::from(n);

        assert_eq!(m.dispatches, n * expected_dispatches(window), "n = {}", n);
        assert_eq!(m.take_next, m.dispatches, "n = {}", n);
        assert_eq!(
            m.update_ready,
            n * (m.dispatches + EXTRA_PASSES),
            "n = {}",
            n
        );
    }
}

/// Sparse regime: the rotation scans not-ready children on every
/// dispatch — Θ(children) probes per dispatch (total n + (n-1)²) and
/// three full readiness scans per wave.
#[tokio::test]
async fn sparse_readiness_probe_cost_matches_the_exact_rotation_model() {
    for n in [10_u32, 100, 200] {
        let m = measure_sparse(n).await;
        let n = u64::from(n);

        assert_eq!(m.dispatches, n, "n = {}", n);
        assert_eq!(m.take_next, n + (n - 1) * (n - 1), "n = {}", n);
        assert_eq!(m.update_ready, 3 * n * n, "n = {}", n);
    }
}
