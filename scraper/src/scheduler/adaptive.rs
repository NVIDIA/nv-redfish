// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::error::Error as StdError;
use std::time::Duration;

use crate::BmcCapacity;

/// Scheduler-observed BMC load state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadState {
    /// No overload signal is currently observed.
    Healthy,
    /// The BMC is responding, but latency has increased sharply.
    Slow,
    /// The BMC has reported overload or a transport timeout/reset.
    Overloaded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestOutcome {
    Success,
    Overload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CapacitySnapshot {
    pub(super) limit: usize,
    pub(super) load_state: LoadState,
}

#[derive(Debug)]
pub(super) struct AdaptiveCapacity {
    adaptive: bool,
    limit: usize,
    max: usize,
    healthy_until_increase: usize,
    baseline_latency: Option<Duration>,
    load_state: LoadState,
}

impl AdaptiveCapacity {
    pub(super) const HEALTHY_WINDOW: usize = 4;
    const SLOW_LATENCY_MULTIPLIER: u32 = 3;

    pub(super) const fn new(capacity: BmcCapacity) -> Self {
        let limit = if capacity.is_adaptive() {
            capacity.initial_in_flight_value()
        } else {
            capacity.max_in_flight_value()
        };
        Self {
            adaptive: capacity.is_adaptive(),
            limit,
            max: capacity.max_in_flight_value(),
            healthy_until_increase: Self::HEALTHY_WINDOW,
            baseline_latency: None,
            load_state: LoadState::Healthy,
        }
    }

    pub(super) const fn limit(&self) -> usize {
        self.limit
    }

    pub(super) const fn load_state(&self) -> LoadState {
        self.load_state
    }

    pub(super) const fn snapshot(&self) -> CapacitySnapshot {
        CapacitySnapshot {
            limit: self.limit,
            load_state: self.load_state,
        }
    }

    pub(super) fn observe(&mut self, latency: Duration, outcome: RequestOutcome) -> bool {
        let before = self.snapshot();
        if !self.adaptive {
            self.record_baseline(latency);
            return before != self.snapshot();
        }
        match outcome {
            RequestOutcome::Success => self.observe_success(latency),
            RequestOutcome::Overload => self.decrease_quickly(LoadState::Overloaded),
        }
        before != self.snapshot()
    }

    fn observe_success(&mut self, latency: Duration) {
        if self.is_sharp_latency_increase(latency) {
            self.decrease_quickly(LoadState::Slow);
            self.record_baseline(latency);
            return;
        }
        self.record_baseline(latency);
        self.load_state = LoadState::Healthy;
        self.healthy_until_increase = self.healthy_until_increase.saturating_sub(1);
        if self.healthy_until_increase == 0 {
            self.limit = (self.limit + 1).min(self.max);
            self.healthy_until_increase = Self::HEALTHY_WINDOW;
        }
    }

    fn decrease_quickly(&mut self, load_state: LoadState) {
        self.limit = (self.limit / 2).max(1);
        self.healthy_until_increase = Self::HEALTHY_WINDOW;
        self.load_state = load_state;
    }

    fn is_sharp_latency_increase(&self, latency: Duration) -> bool {
        self.baseline_latency.is_some_and(|baseline| {
            baseline > Duration::ZERO
                && latency >= baseline.saturating_mul(Self::SLOW_LATENCY_MULTIPLIER)
        })
    }

    fn record_baseline(&mut self, latency: Duration) {
        self.baseline_latency = Some(
            self.baseline_latency
                .map_or(latency, |baseline| (baseline + latency) / 2),
        );
    }
}

pub(super) fn classify_error(error: &(dyn StdError + Send + Sync)) -> RequestOutcome {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timeout")
        || message.contains("timed out")
        || message.contains("429")
        || message.contains("503")
        || message.contains("connection reset")
        || message.contains("reset by peer")
    {
        RequestOutcome::Overload
    } else {
        RequestOutcome::Success
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::AdaptiveCapacity;
    use super::LoadState;
    use super::RequestOutcome;
    use crate::BmcCapacity;

    #[test]
    fn adaptive_capacity_starts_conservative() {
        let adaptive = AdaptiveCapacity::new(
            BmcCapacity::adaptive()
                .initial_in_flight(1)
                .max_in_flight(16),
        );

        assert_eq!(adaptive.limit(), 1);
    }

    #[test]
    fn adaptive_capacity_increases_after_healthy_window() {
        let mut adaptive = AdaptiveCapacity::new(
            BmcCapacity::adaptive()
                .initial_in_flight(1)
                .max_in_flight(16),
        );

        for _ in 0..AdaptiveCapacity::HEALTHY_WINDOW {
            let _changed = adaptive.observe(Duration::from_millis(10), RequestOutcome::Success);
        }

        assert_eq!(adaptive.limit(), 2);
        assert_eq!(adaptive.load_state(), LoadState::Healthy);
    }

    #[test]
    fn adaptive_capacity_decreases_after_timeout() {
        let mut adaptive = AdaptiveCapacity::new(
            BmcCapacity::adaptive()
                .initial_in_flight(8)
                .max_in_flight(16),
        );

        let _changed = adaptive.observe(Duration::from_millis(10), RequestOutcome::Overload);

        assert_eq!(adaptive.limit(), 4);
        assert_eq!(adaptive.load_state(), LoadState::Overloaded);
    }

    #[test]
    fn adaptive_capacity_marks_load_state_slow() {
        let mut adaptive = AdaptiveCapacity::new(
            BmcCapacity::adaptive()
                .initial_in_flight(4)
                .max_in_flight(16),
        );
        let _changed = adaptive.observe(Duration::from_millis(10), RequestOutcome::Success);

        let _changed = adaptive.observe(Duration::from_millis(40), RequestOutcome::Success);

        assert_eq!(adaptive.limit(), 2);
        assert_eq!(adaptive.load_state(), LoadState::Slow);
    }
}
