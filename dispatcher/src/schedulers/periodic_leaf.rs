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

//! Interval-driven work leaf.
//!
//! Fires one payload from a factory closure each time its interval
//! elapses, replacing hand-rolled poll loops. The next tick is scheduled
//! from *dispatch* time (fixed delay), so a leaf held back by upstream
//! policy — a token bucket, a tripped circuit breaker — does not burst to
//! catch up once released. While waiting, `update_ready` reports the due
//! instant so the runtime can sleep precisely.
//!
//! Dispatching does not wait for the previous item to complete: a payload
//! that runs longer than the interval overlaps the next one. Wrap the
//! leaf in [`crate::schedulers::BoundedConcurrency`] with a cap of 1 to
//! serialize iterations.

use core::marker::PhantomData;
use core::time::Duration;
use std::time::Instant;

use crate::scheduler::{ScheduledWork, Scheduler};
use crate::work::{Completion, Readiness};

/// When the next item becomes due.
enum Due {
    /// Due immediately (initial state: nothing dispatched yet).
    Now,
    /// Due at the given instant.
    At(Instant),
    /// Never due again
    Never,
}

/// Leaf that produces `make()` every `interval`. Meta is `()`; compose
/// with [`crate::schedulers::FixedCost`] to sit under cost-aware
/// branches.
pub struct PeriodicLeaf<T, F> {
    interval: Duration,
    next_due: Due,
    last_now: Instant,
    make: F,
    _t: PhantomData<fn() -> T>,
}

impl<T, F> PeriodicLeaf<T, F>
where
    F: FnMut() -> T + Send + 'static,
{
    /// Leaf firing `make()` every `interval`, starting immediately.
    /// `now` is the scheduling epoch — pass the driving clock's current
    /// time.
    ///
    /// An `interval` too large for `Instant` arithmetic (e.g.
    /// [`Duration::MAX`]) fires the first item and then never again.
    #[must_use]
    pub fn new(now: Instant, interval: Duration, make: F) -> Self {
        Self {
            interval,
            next_due: Due::Now,
            last_now: now,
            make,
            _t: PhantomData,
        }
    }

    /// Leaf whose first item is due at `first_due` instead of
    /// immediately. Use to stagger a fleet of leaves sharing an interval.
    #[must_use]
    pub fn starting_at(now: Instant, first_due: Instant, interval: Duration, make: F) -> Self {
        Self {
            interval,
            next_due: Due::At(first_due),
            last_now: now,
            make,
            _t: PhantomData,
        }
    }

    /// Configured interval.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// Change the interval. Takes effect when the next item is
    /// dispatched; an already-scheduled due time is left as is.
    pub const fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn due(&self, now: Instant) -> bool {
        match self.next_due {
            Due::Now => true,
            Due::At(d) => now >= d,
            Due::Never => false,
        }
    }
}

impl<T, F> Scheduler<T> for PeriodicLeaf<T, F>
where
    T: Send + 'static,
    F: FnMut() -> T + Send + 'static,
{
    type Meta = ();

    fn update_ready(&mut self, now: Instant) -> Readiness {
        self.last_now = now;
        if self.due(now) {
            Readiness::ready(None)
        } else {
            let hint = match self.next_due {
                Due::At(d) => Some(d),
                Due::Now | Due::Never => None,
            };
            Readiness::not_ready(hint)
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<T, ()>> {
        if !self.due(self.last_now) {
            return None;
        }

        self.next_due = self
            .last_now
            .checked_add(self.interval)
            .map_or(Due::Never, Due::At);
        Some(ScheduledWork::new((), (self.make)()))
    }

    fn on_complete(&mut self, _completion: Completion<()>) {}
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use core::time::Duration;
    use std::time::Instant;

    use super::PeriodicLeaf;
    use crate::scheduler::Scheduler as _;

    #[test]
    fn fires_immediately_then_respects_interval() {
        let interval = Duration::from_secs(5);
        let t0 = Instant::now();
        let mut leaf = PeriodicLeaf::new(t0, interval, || 42_u64);

        assert!(leaf.update_ready(t0).ready);
        let work = leaf.take_next().expect("due immediately");
        assert_eq!(work.payload, 42);

        // Not due again until one interval after dispatch.
        let r = leaf.update_ready(t0);
        assert!(!r.ready);
        assert_eq!(r.next_update_at, Some(t0 + interval));
        assert!(leaf.take_next().is_none());

        assert!(leaf.update_ready(t0 + interval).ready);
        assert!(leaf.take_next().is_some());
    }

    #[test]
    fn no_catch_up_burst_after_a_long_stall() {
        let interval = Duration::from_secs(1);
        let t0 = Instant::now();
        let mut leaf = PeriodicLeaf::new(t0, interval, || 1_u64);
        leaf.update_ready(t0);
        leaf.take_next().expect("first tick");

        // 10 intervals pass unserved; exactly one item is due, and the
        // next is a full interval after that dispatch.
        let t1 = t0 + Duration::from_secs(10);
        assert!(leaf.update_ready(t1).ready);
        leaf.take_next().expect("one catch-up item only");
        let r = leaf.update_ready(t1);
        assert!(!r.ready);
        assert_eq!(r.next_update_at, Some(t1 + interval));
    }

    #[test]
    fn take_next_without_due_tick_yields_nothing() {
        let t0 = Instant::now();
        let mut leaf = PeriodicLeaf::new(t0, Duration::from_secs(1), || 1_u64);
        leaf.update_ready(t0);
        assert!(leaf.take_next().is_some());
        // A branch probing again in the same pass gets nothing.
        assert!(leaf.take_next().is_none());
    }

    #[test]
    fn overflowing_interval_fires_once_then_never_without_panicking() {
        let t0 = Instant::now();
        let mut leaf = PeriodicLeaf::new(t0, Duration::MAX, || 1_u64);
        assert!(leaf.update_ready(t0).ready);
        assert!(leaf.take_next().is_some(), "first tick fires");

        // Instant + Duration::MAX overflows: the leaf must go dormant
        // (not-ready, no hint) instead of panicking in the runtime.
        let r = leaf.update_ready(t0 + Duration::from_secs(1));
        assert!(!r.ready);
        assert!(r.next_update_at.is_none());
        assert!(leaf.take_next().is_none());
    }

    #[test]
    fn starting_at_delays_the_first_item() {
        let interval = Duration::from_secs(1);
        let t0 = Instant::now();
        let first = t0 + Duration::from_millis(300);
        let mut leaf = PeriodicLeaf::starting_at(t0, first, interval, || 1_u64);

        let r = leaf.update_ready(t0);
        assert!(!r.ready);
        assert_eq!(r.next_update_at, Some(first));

        assert!(leaf.update_ready(first).ready);
        assert!(leaf.take_next().is_some());
    }
}
