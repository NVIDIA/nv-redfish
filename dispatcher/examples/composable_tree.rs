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

//! Composable scheduler tree + runtime + dynamic mutation.
//!
//! Tree we build at startup:
//!
//! ```text
//!     RoundRobin (root, Meta = ())
//!     ├─ PollLeaf("sensor-A")
//!     └─ PollLeaf("sensor-B")
//! ```
//!
//! Then we mutate it at runtime through `RuntimeHandle::with_root_mut`:
//!
//! - add a third leaf to the existing root,
//! - add a sub-branch with two more leaves under the root.
//!
//! Final shape:
//!
//! ```text
//!     RoundRobin (root)
//!     ├─ PollLeaf("sensor-A")
//!     ├─ PollLeaf("sensor-B")
//!     ├─ PollLeaf("sensor-C")
//!     └─ RoundRobin (subnet)
//!        ├─ PollLeaf("subnet-1-a")
//!        └─ PollLeaf("subnet-1-b")
//! ```
//!
//! The runtime never sees this tree; it just drives the root. Children are
//! added through the branch's own API, reached by downcasting the root to
//! its concrete type.
//!
//! The `RoundRobin` and `PollLeaf` here are hand-rolled to show how to
//! write custom nodes; production trees can compose the built-ins from
//! `nv_redfish_dispatcher::schedulers` instead.
//!
//! A background task calls `graceful_shutdown` after a few seconds; the
//! driver drains, prints runtime stats, and exits. The driver honors
//! `SleepUntil` with a plain sleep, so a shutdown issued mid-sleep is
//! observed at the next `next()` call (see `RuntimeOutput::SleepUntil`).

use core::marker::PhantomData;
use core::time::Duration;
use nv_redfish_dispatcher::ClockConfig;
use nv_redfish_dispatcher::Completion;
use nv_redfish_dispatcher::FutureWork;
use nv_redfish_dispatcher::Readiness;
use nv_redfish_dispatcher::Runtime;
use nv_redfish_dispatcher::RuntimeConfig;
use nv_redfish_dispatcher::RuntimeOutput;
use nv_redfish_dispatcher::ScheduledWork;
use nv_redfish_dispatcher::Scheduler;
use nv_redfish_dispatcher::WorkMeta;
use std::num::NonZero;
use std::time::Instant;

// ──────────────────────────────────────────────────────────────────────
// Application types
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
#[allow(dead_code)] // fields are consumed only via Debug in this demo
enum Event {
    Polled { source: String },
}

#[derive(Debug)]
#[allow(dead_code)]
struct Error(String);

// Convenience alias for "the payload this runtime executes".
type Work = FutureWork<Event, Error>;

// ──────────────────────────────────────────────────────────────────────
// Leaf: periodically emits one Event::Polled
// ──────────────────────────────────────────────────────────────────────

struct PollLeaf {
    name: String,
    interval: Duration,
    next_due: Option<Instant>,
}

impl PollLeaf {
    fn new(name: impl Into<String>, interval: Duration) -> Self {
        Self {
            name: name.into(),
            interval,
            next_due: None,
        }
    }
}

impl Scheduler<Work> for PollLeaf {
    type Meta = ();

    fn update_ready(&mut self, now: Instant) -> Readiness {
        match self.next_due {
            Some(due) if now < due => Readiness::not_ready(Some(due)),
            _ => Readiness::ready(None),
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<Work, ()>> {
        let now = Instant::now();
        self.next_due = Some(now + self.interval);

        let name = self.name.clone();
        let payload: Work = Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            Ok(vec![Event::Polled { source: name }])
        });

        Some(ScheduledWork::new((), payload))
    }

    fn on_complete(&mut self, _: Completion<()>) {
        // Leaves typically use this to update local stats / backoff.
    }
}

// ──────────────────────────────────────────────────────────────────────
// Branch: round-robin over heterogeneous children
// ──────────────────────────────────────────────────────────────────────
//
// Children are stored type-erased so leaves and sub-branches mix freely as
// long as they share the same payload `T` and meta `M`.

struct RoundRobin<T, M: WorkMeta> {
    children: Vec<Box<dyn Scheduler<T, Meta = M>>>,
    cursor: usize,
    _payload: PhantomData<fn() -> T>,
}

impl<T, M: WorkMeta> RoundRobin<T, M> {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            cursor: 0,
            _payload: PhantomData,
        }
    }

    fn add_child<S: Scheduler<T, Meta = M>>(&mut self, child: S) {
        self.children.push(Box::new(child));
    }
}

impl<T, M> Scheduler<T> for RoundRobin<T, M>
where
    T: Send + 'static,
    M: WorkMeta,
{
    // RoundRobin is policy-light: it doesn't add a meta layer, just forwards
    // the child's meta upward.
    type Meta = M;

    fn update_ready(&mut self, now: Instant) -> Readiness {
        let mut ready = false;
        let mut next_at: Option<Instant> = None;
        for child in &mut self.children {
            let r = child.update_ready(now);
            ready |= r.ready;
            next_at = match (next_at, r.next_update_at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
        Readiness {
            ready,
            next_update_at: next_at,
            next_cost: None,
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<T, Self::Meta>> {
        let n = self.children.len();
        if n == 0 {
            return None;
        }
        // Try every child once before giving up; honours `take_next`'s
        // "may return None, keep scanning" contract.
        for _ in 0..n {
            let idx = self.cursor;
            self.cursor = (self.cursor + 1) % n;
            if let Some(mut work) = self.children[idx].take_next() {
                // Branch contract: stamp the chosen child index.
                work.routing.push(idx as u32);
                return Some(work);
            }
        }
        None
    }

    fn on_complete(&mut self, mut completion: Completion<Self::Meta>) {
        // Branch contract: pop our tag and forward to the originating child.
        if let Some(idx) = completion.routing.pop() {
            if let Some(child) = self.children.get_mut(idx as usize) {
                child.on_complete(completion);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Driver
// ──────────────────────────────────────────────────────────────────────

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // 1. Build the initial tree.
    let mut root: RoundRobin<Work, ()> = RoundRobin::new();
    root.add_child(PollLeaf::new("sensor-A", Duration::from_secs(1)));
    root.add_child(PollLeaf::new("sensor-B", Duration::from_secs(2)));
    let mut subnet: RoundRobin<Work, ()> = RoundRobin::new();
    subnet.add_child(PollLeaf::new("static-subnet-1-a", Duration::from_secs(5)));
    subnet.add_child(PollLeaf::new("static-subnet-1-b", Duration::from_secs(5)));
    root.add_child(subnet);

    // 2. Hand the root to the runtime. The runtime takes ownership and
    //    boxes it behind an internal mutex.
    let mut runtime: Runtime<Event, Error, ()> = Runtime::new(
        RuntimeConfig {
            global_max_in_flight: NonZero::<usize>::MIN,
            clock: ClockConfig::Wallclock,
        },
        root,
    );
    let handle = runtime.handle();

    // 3a. Add another leaf to the root, dynamically.
    handle
        .with_root_mut(|root: &mut RoundRobin<Work, ()>| {
            root.add_child(PollLeaf::new("sensor-C", Duration::from_secs(3)));
        })
        .expect("downcast failed");

    // 3b. Add an entirely new sub-branch with its own children.
    handle
        .with_root_mut(|root: &mut RoundRobin<Work, ()>| {
            let mut subnet: RoundRobin<Work, ()> = RoundRobin::new();
            subnet.add_child(PollLeaf::new("subnet-1-a", Duration::from_secs(5)));
            subnet.add_child(PollLeaf::new("subnet-1-b", Duration::from_secs(5)));
            root.add_child(subnet);
        })
        .expect("downcast failed");

    // 4. Shut down from the outside after a few seconds. In-flight work
    //    still drains; `next()` then emits a sticky `Shutdown`.
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(6)).await;
        shutdown_handle.graceful_shutdown();
    });

    // 5. Drive the runtime. `next()` is the single ordered output stream.
    let start = Instant::now();
    loop {
        match runtime.next().await {
            RuntimeOutput::SleepUntil(v) => {
                tokio::time::sleep(v.duration_since(Instant::now())).await;
            }
            RuntimeOutput::Work { result, latency } => match result {
                Ok(events) => println!(
                    "{:?} got {:?} events in {:?}",
                    Instant::now().duration_since(start),
                    events,
                    latency
                ),
                Err(Error(msg)) => eprintln!("work failed in {:?}: {}", latency, msg),
            },
            RuntimeOutput::Runtime(_) => {
                // Out-of-band runtime events; only constructible with the
                // `runtime-events` feature. Ignored here.
            }
            RuntimeOutput::Shutdown => break,
        }
    }

    let stats = handle.stats();
    println!(
        "shut down after {:?}: {} items dispatched",
        Instant::now().duration_since(start),
        stats.dispatched
    );
}
