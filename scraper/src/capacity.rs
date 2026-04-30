// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

/// Scheduler capacity policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmcCapacity {
    mode: CapacityMode,
    initial_in_flight: usize,
    max_in_flight: usize,
    max_requests_per_second: u32,
    interactive_share: u8,
    subscription_share: u8,
    discovery_share: u8,
    maintenance_share: u8,
}

impl BmcCapacity {
    /// Creates the default adaptive capacity policy.
    #[must_use]
    pub const fn adaptive() -> Self {
        Self {
            mode: CapacityMode::Adaptive,
            initial_in_flight: 1,
            max_in_flight: 16,
            max_requests_per_second: 30,
            interactive_share: 50,
            subscription_share: 30,
            discovery_share: 15,
            maintenance_share: 5,
        }
    }

    /// Creates the default fixed capacity policy.
    #[must_use]
    pub const fn fixed() -> Self {
        Self {
            mode: CapacityMode::Fixed,
            initial_in_flight: 1,
            max_in_flight: 1,
            max_requests_per_second: 1,
            interactive_share: 50,
            subscription_share: 30,
            discovery_share: 15,
            maintenance_share: 5,
        }
    }

    /// Sets the initial in-flight request limit recorded for adaptive policy.
    #[must_use]
    pub const fn initial_in_flight(mut self, value: usize) -> Self {
        self.initial_in_flight = value;
        self
    }

    /// Sets the maximum in-flight request limit.
    #[must_use]
    pub const fn max_in_flight(mut self, value: usize) -> Self {
        self.max_in_flight = value;
        self
    }

    /// Sets the maximum request rate.
    #[must_use]
    pub const fn max_requests_per_second(mut self, value: u32) -> Self {
        self.max_requests_per_second = value;
        self
    }

    /// Sets the scheduler share recorded for interactive work.
    #[must_use]
    pub const fn interactive_share(mut self, value: u8) -> Self {
        self.interactive_share = value;
        self
    }

    /// Sets the scheduler share recorded for subscription work.
    #[must_use]
    pub const fn subscription_share(mut self, value: u8) -> Self {
        self.subscription_share = value;
        self
    }

    /// Sets the scheduler share recorded for discovery work.
    #[must_use]
    pub const fn discovery_share(mut self, value: u8) -> Self {
        self.discovery_share = value;
        self
    }

    /// Sets the scheduler share recorded for maintenance work.
    #[must_use]
    pub const fn maintenance_share(mut self, value: u8) -> Self {
        self.maintenance_share = value;
        self
    }

    pub(crate) const fn max_in_flight_value(self) -> usize {
        if self.max_in_flight == 0 {
            1
        } else {
            self.max_in_flight
        }
    }

    pub(crate) const fn max_requests_per_second_value(self) -> u32 {
        if self.max_requests_per_second == 0 {
            1
        } else {
            self.max_requests_per_second
        }
    }

    pub(crate) const fn lane_share(self, lane: crate::Lane) -> u32 {
        let configured = match lane {
            crate::Lane::Interactive => self.interactive_share,
            crate::Lane::Subscription => self.subscription_share,
            crate::Lane::Discovery => self.discovery_share,
            crate::Lane::Maintenance => self.maintenance_share,
        };
        if configured == 0 {
            1
        } else {
            configured as u32
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapacityMode {
    Adaptive,
    Fixed,
}

impl BmcCapacity {
    pub(crate) const fn is_adaptive(self) -> bool {
        matches!(self.mode, CapacityMode::Adaptive)
    }

    pub(crate) const fn initial_in_flight_value(self) -> usize {
        let initial = if self.initial_in_flight == 0 {
            1
        } else {
            self.initial_in_flight
        };
        if initial > self.max_in_flight_value() {
            self.max_in_flight_value()
        } else {
            initial
        }
    }
}
