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

//! Circuit-breaker scheduler with Closed / Open / `HalfOpen` states.
//!
//! Maintains a bounded ring of recent outcomes; trips the breaker when
//! the failure fraction exceeds the configured threshold. While open,
//! `take_next` returns `None` and `update_ready` reports the open-until
//! deadline. After `cool_down` elapses the breaker enters `HalfOpen`,
//! admits up to `half_open_max_probes`, and on the first probe outcome
//! either resets to Closed (success) or re-opens with a fresh cool-down
//! (failure).

use core::convert::TryFrom as _;
use core::marker::PhantomData;
use core::time::Duration;
use std::collections::VecDeque;
use std::time::Instant;

use crate::scheduler::{ScheduledWork, Scheduler};
use crate::work::{Completion, CompletionOutcome, Readiness};

/// Configuration for a [`CircuitBreaker`].
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Failure fraction in `0.0..=1.0` that trips the breaker.
    pub failure_threshold: f32,
    /// Capacity of the rolling outcomes window.
    pub sample_window: u32,
    /// Minimum samples required before the threshold is evaluated.
    pub min_samples: u32,
    /// How long the breaker stays open before going `HalfOpen`.
    pub cool_down: Duration,
    /// Concurrent probes allowed in `HalfOpen`.
    pub half_open_max_probes: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 0.5,
            sample_window: 32,
            min_samples: 5,
            cool_down: Duration::from_secs(10),
            half_open_max_probes: 1,
        }
    }
}

/// Circuit-breaker state. Exposed for inspection via
/// [`CircuitBreaker::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Closed: forwarding everything; recording outcomes.
    Closed,
    /// Open until the given instant.
    Open {
        /// Wall-clock time after which the breaker may move to
        /// `HalfOpen`.
        until: Instant,
    },
    /// `HalfOpen`: admitting up to `half_open_max_probes` outstanding
    /// items.
    HalfOpen {
        /// Probes currently outstanding.
        in_flight: u32,
    },
}

/// Circuit-breaker decorator wrapping a single child scheduler
pub struct CircuitBreaker<T, C: Scheduler<T>> {
    inner: C,
    state: BreakerState,
    cfg: CircuitBreakerConfig,
    samples: VecDeque<CompletionOutcome>,
    last_now: Instant,
    _t: PhantomData<fn() -> T>,
}

impl<T, C: Scheduler<T>> CircuitBreaker<T, C> {
    #[must_use]
    /// Create new CircuitBreaker with the given config and child scheduler
    pub fn new(cfg: CircuitBreakerConfig, child: C) -> Self {
        Self {
            inner: child,
            state: BreakerState::Closed,
            cfg,
            samples: VecDeque::new(),
            last_now: Instant::now(),
            _t: PhantomData,
        }
    }

    /// Current breaker state
    #[must_use]
    pub const fn state(&self) -> BreakerState {
        self.state
    }

    fn record_outcome(&mut self, outcome: CompletionOutcome) {
        let cap = self.cfg.sample_window as usize;
        if cap == 0 {
            return;
        }
        if self.samples.len() >= cap {
            self.samples.pop_front();
        }
        self.samples.push_back(outcome);
    }

    fn failure_rate(&self) -> Option<f32> {
        let total = u32::try_from(self.samples.len()).unwrap_or(u32::MAX);
        if total < self.cfg.min_samples {
            return None;
        }
        let failures = u32::try_from(
            self.samples
                .iter()
                .filter(|o| matches!(o, CompletionOutcome::Failed))
                .count(),
        )
        .unwrap_or(u32::MAX);

        // This is fine our our sample sizes
        #[allow(clippy::cast_precision_loss)]
        let rate = failures as f32 / total as f32;
        Some(rate)
    }

    fn maybe_trip(&mut self) {
        if let Some(rate) = self.failure_rate() {
            if rate >= self.cfg.failure_threshold {
                self.state = BreakerState::Open {
                    until: self.last_now + self.cfg.cool_down,
                };
                self.samples.clear();
            }
        }
    }
}

impl<T, C> Scheduler<T> for CircuitBreaker<T, C>
where
    T: Send + 'static,
    C: Scheduler<T>,
{
    type Meta = C::Meta;

    fn update_ready(&mut self, now: Instant) -> Readiness {
        self.last_now = now;
        if let BreakerState::Open { until } = self.state {
            if now >= until {
                self.state = BreakerState::HalfOpen { in_flight: 0 };
            } else {
                return Readiness::not_ready(Some(until));
            }
        }
        match self.state {
            BreakerState::Closed => self.inner.update_ready(now),
            BreakerState::Open { until } => Readiness::not_ready(Some(until)),
            BreakerState::HalfOpen { in_flight } => {
                if in_flight >= self.cfg.half_open_max_probes {
                    Readiness::not_ready(None)
                } else {
                    self.inner.update_ready(now)
                }
            }
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<T, C::Meta>> {
        match &mut self.state {
            BreakerState::Open { .. } => None,
            BreakerState::Closed => self.inner.take_next(),
            BreakerState::HalfOpen { in_flight } => {
                if *in_flight >= self.cfg.half_open_max_probes {
                    return None;
                }
                let work = self.inner.take_next()?;
                *in_flight = in_flight.saturating_add(1);
                Some(work)
            }
        }
    }

    fn on_complete(&mut self, completion: Completion<C::Meta>) {
        let outcome = completion.outcome;
        match self.state {
            BreakerState::Closed => {
                self.record_outcome(outcome);
                self.maybe_trip();
            }
            BreakerState::Open { .. } => {
                // We do not change state here
            }
            BreakerState::HalfOpen { .. } => {
                self.state = match outcome {
                    CompletionOutcome::Succeeded => {
                        self.samples.clear();
                        BreakerState::Closed
                    }
                    CompletionOutcome::Failed => {
                        self.samples.clear();
                        BreakerState::Open {
                            until: self.last_now + self.cfg.cool_down,
                        }
                    }
                };
            }
        }
        self.inner.on_complete(completion);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use core::time::Duration;
    use std::time::Instant;

    use super::{BreakerState, CircuitBreaker, CircuitBreakerConfig};
    use crate::scheduler::Scheduler as _;
    use crate::schedulers::tests::{MockLeaf, TestPayload};
    use crate::work::{Completion, CompletionOutcome};

    fn cfg(cool_down: Duration) -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 0.5,
            sample_window: 10,
            min_samples: 4,
            cool_down,
            half_open_max_probes: 1,
        }
    }

    fn drive_outcome(
        cb: &mut CircuitBreaker<TestPayload, MockLeaf<()>>,
        now: Instant,
        outcome: CompletionOutcome,
    ) {
        let r = cb.update_ready(now);
        if !r.ready {
            return;
        }
        let Some(work) = cb.take_next() else {
            return;
        };
        cb.on_complete(Completion {
            outcome,
            latency: Duration::ZERO,
            meta: work.meta,
            routing: work.routing,
        });
    }

    #[test]
    fn closed_passes_through_until_failure_threshold() {
        let leaf = MockLeaf::ready_firing(1);
        let mut cb = CircuitBreaker::new(cfg(Duration::from_millis(100)), leaf);
        let t0 = Instant::now();

        // 4 failures in a row at 50% threshold trip it.
        for _ in 0..4 {
            drive_outcome(&mut cb, t0, CompletionOutcome::Failed);
        }
        assert!(matches!(cb.state(), BreakerState::Open { .. }));

        // While open, take_next returns None and update_ready hints
        // the open-until time.
        let r = cb.update_ready(t0);
        assert!(!r.ready);
        let until = r.next_update_at.expect("open hint");
        assert!(until > t0);
        assert!(cb.take_next().is_none());
    }

    #[test]
    fn cool_down_promotes_to_half_open() {
        let leaf = MockLeaf::ready_firing(1);
        let cool_down = Duration::from_millis(50);
        let mut cb = CircuitBreaker::new(cfg(cool_down), leaf);
        let t0 = Instant::now();

        for _ in 0..4 {
            drive_outcome(&mut cb, t0, CompletionOutcome::Failed);
        }
        assert!(matches!(cb.state(), BreakerState::Open { .. }));

        // After cool_down, update_ready must transition to HalfOpen.
        let t1 = t0 + cool_down + Duration::from_millis(1);
        let r = cb.update_ready(t1);
        assert!(matches!(cb.state(), BreakerState::HalfOpen { .. }));
        assert!(r.ready);
    }

    #[test]
    fn half_open_success_closes_breaker() {
        let leaf = MockLeaf::ready_firing(1);
        let cool_down = Duration::from_millis(50);
        let mut cb = CircuitBreaker::new(cfg(cool_down), leaf);
        let t0 = Instant::now();
        for _ in 0..4 {
            drive_outcome(&mut cb, t0, CompletionOutcome::Failed);
        }

        let t1 = t0 + cool_down + Duration::from_millis(1);
        drive_outcome(&mut cb, t1, CompletionOutcome::Succeeded);
        assert!(matches!(cb.state(), BreakerState::Closed));
    }

    #[test]
    fn half_open_failure_re_opens_with_fresh_cool_down() {
        let leaf = MockLeaf::ready_firing(1);
        let cool_down = Duration::from_millis(50);
        let mut cb = CircuitBreaker::new(cfg(cool_down), leaf);
        let t0 = Instant::now();
        for _ in 0..4 {
            drive_outcome(&mut cb, t0, CompletionOutcome::Failed);
        }

        let t1 = t0 + cool_down + Duration::from_millis(1);
        drive_outcome(&mut cb, t1, CompletionOutcome::Failed);
        match cb.state() {
            BreakerState::Open { until } => {
                assert!(until > t1);
            }
            other => unreachable!("expected Open, got {:?}", other),
        }
    }

    #[test]
    fn half_open_caps_concurrent_probes() {
        // half_open_max_probes = 1: once a probe is in-flight, the
        // breaker reports not-ready until that probe completes.
        let leaf = MockLeaf::ready_firing(1);
        let cool_down = Duration::from_millis(10);
        let mut cb = CircuitBreaker::new(cfg(cool_down), leaf);
        let t0 = Instant::now();
        for _ in 0..4 {
            drive_outcome(&mut cb, t0, CompletionOutcome::Failed);
        }
        let t1 = t0 + cool_down + Duration::from_millis(1);
        cb.update_ready(t1);
        assert!(matches!(cb.state(), BreakerState::HalfOpen { .. }));

        // Take one probe; do NOT complete it yet.
        let probe = cb.take_next().expect("first probe admitted");
        assert!(!cb.update_ready(t1).ready);
        assert!(cb.take_next().is_none());

        // Drain the probe successfully so we don't leak meta.
        cb.on_complete(Completion {
            outcome: CompletionOutcome::Succeeded,
            latency: Duration::ZERO,
            meta: probe.meta,
            routing: probe.routing,
        });
        assert!(matches!(cb.state(), BreakerState::Closed));
    }
}
