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

//! Scheduling-invariant scenarios on the shared virtual-time harness.

use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use std::sync::Arc;

use nv_redfish_dispatcher::{ManualClock, RoundRobin};
use nv_redfish_dispatcher_sim::{
    add_sources, ample_bucket, assert_interval_exact, breaker, cost_of, count, scarce_bucket,
    simulate, Dispatch, TASKS,
};

/// Half the sources have rate headroom, half are throttled below their
/// demand. The unconstrained half must stay interval-exact; the
/// throttled half must stay within its cost budget.
#[tokio::test]
async fn throttled_sources_do_not_disturb_the_rest() {
    const AMPLE: u32 = 40;
    const SCARCE: u32 = 40;
    let window = Duration::from_secs(600);

    let clock = ManualClock::new();
    let now = clock.now();
    let no_fail = Arc::new(AtomicBool::new(false));
    let mut root = RoundRobin::new();
    add_sources(&mut root, now, 0..AMPLE, ample_bucket(), &no_fail);
    add_sources(
        &mut root,
        now,
        AMPLE..AMPLE + SCARCE,
        scarce_bucket(),
        &no_fail,
    );

    let log = simulate(clock, root, window, Vec::new()).await;

    assert_interval_exact(&log, 0..AMPLE, window);

    // Bucket-paced budget: capacity + refill over the window, give or
    // take one item of cost (the admission gate is one token).
    let cfg = scarce_bucket();
    let refill = window.as_secs() / cfg.refill_interval.as_secs() * cfg.refill_amount.get();
    let max_cost = TASKS.iter().map(|t| t.cost).max().unwrap_or(0);
    let budget = (refill - max_cost)..=(cfg.capacity.get() + refill + max_cost);
    for idx in AMPLE..AMPLE + SCARCE {
        let spent: u64 = log
            .iter()
            .filter(|d| d.source == idx)
            .map(|d| cost_of(d.task))
            .sum();
        assert!(budget.contains(&spent), "source {}: {} units", idx, spent);
        assert_eq!(count(&log, |d| d.source == idx && !d.ok), 0);
    }
}

/// A few sources fail hard for a while: their breakers must collapse
/// attempts to probes, the healthy sources must stay interval-exact,
/// and the failed sources must resume within one cool-down of recovery.
#[tokio::test]
async fn breaker_isolates_an_outage_and_recovers() {
    const HEALTHY: u32 = 15;
    const FAILING: u32 = 5;
    let window = Duration::from_secs(600);
    let outage = Duration::from_secs(200)..Duration::from_secs(400);

    let clock = ManualClock::new();
    let now = clock.now();
    let no_fail = Arc::new(AtomicBool::new(false));
    let mut root = RoundRobin::new();
    add_sources(&mut root, now, 0..HEALTHY, ample_bucket(), &no_fail);
    let flags: Vec<_> = (HEALTHY..HEALTHY + FAILING)
        .map(|idx| {
            let flag = Arc::new(AtomicBool::new(false));
            root.add_child(nv_redfish_dispatcher_sim::source(
                now,
                idx,
                ample_bucket(),
                flag.clone(),
            ));
            flag
        })
        .collect();

    let set = flags.clone();
    let clear = flags;
    let actions: Vec<(Duration, Box<dyn FnOnce() + Send>)> = vec![
        (
            outage.start,
            Box::new(move || set.iter().for_each(|f| f.store(true, Ordering::Relaxed))),
        ),
        (
            outage.end,
            Box::new(move || clear.iter().for_each(|f| f.store(false, Ordering::Relaxed))),
        ),
    ];

    let log = simulate(clock, root, window, actions).await;

    assert_interval_exact(&log, 0..HEALTHY, window);

    let cool_down = breaker().cool_down;
    let frequent = TASKS[0];
    let resumed_by = outage.end + cool_down;
    let resumed_fires = (window - resumed_by).as_secs() / frequent.interval.as_secs();
    for idx in HEALTHY..HEALTHY + FAILING {
        // Trip window plus one half-open probe per cool-down; without
        // the breaker the source would attempt ~26 dispatches.
        let attempts = count(&log, |d| d.source == idx && outage.contains(&d.at) && !d.ok);
        let max_attempts = u64::from(breaker().min_samples)
            + (outage.end - outage.start).as_secs() / cool_down.as_secs()
            + 5;
        assert!(
            attempts >= u64::from(breaker().min_samples) && attempts <= max_attempts,
            "source {}: {} attempts during outage",
            idx,
            attempts
        );
        assert_eq!(
            count(&log, |d| d.source == idx && outage.contains(&d.at) && d.ok),
            0,
            "source {}: no successes during outage",
            idx
        );

        let recovered = count(&log, |d: &Dispatch| {
            d.source == idx && d.task == frequent.id && d.ok && d.at >= resumed_by
        });
        assert!(
            recovered >= resumed_fires - 2,
            "source {}: {} fires after recovery",
            idx,
            recovered
        );
    }
}

/// Full scale, uniform sources, short window. O(children) readiness
/// scans make this slow in debug builds; run with
/// `cargo test --release -- --ignored`.
#[tokio::test]
#[ignore = "release-mode scale run"]
async fn ten_thousand_sources_are_interval_exact() {
    const N: u32 = 10_000;
    let window = Duration::from_secs(60);

    let clock = ManualClock::new();
    let now = clock.now();
    let no_fail = Arc::new(AtomicBool::new(false));
    let mut root = RoundRobin::new();
    add_sources(&mut root, now, 0..N, ample_bucket(), &no_fail);

    let log = simulate(clock, root, window, Vec::new()).await;

    assert_interval_exact(&log, 0..N, window);
}
