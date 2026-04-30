// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::future::ready;
use std::future::Ready;

use crate::discovery::DiscoveryRegistry;
use crate::BmcCapacity;
use crate::Discovery;
use crate::Error;
use crate::Scraper;

/// Builder for [`Scraper`].
///
/// Stores configuration and registered discovery strategies without issuing BMC
/// requests during construction.
#[derive(Debug)]
pub struct ScraperBuilder<B> {
    bmc: B,
    capacity: BmcCapacity,
    discovery: DiscoveryRegistry,
}

impl<B> ScraperBuilder<B> {
    pub(crate) fn new(bmc: B) -> Self {
        Self {
            bmc,
            capacity: BmcCapacity::adaptive(),
            discovery: DiscoveryRegistry::default(),
        }
    }

    /// Sets the scheduler capacity policy.
    #[must_use]
    pub const fn capacity(mut self, capacity: BmcCapacity) -> Self {
        self.capacity = capacity;
        self
    }

    /// Registers a discovery bundle.
    ///
    /// Registration is side-effect free. It does not crawl the BMC.
    #[must_use]
    pub fn discover(mut self, discovery: Discovery) -> Self {
        self.discovery.register(discovery);
        self
    }

    /// Builds a scraper future.
    ///
    /// The returned future is immediately ready. Construction does not start
    /// background work or issue BMC requests.
    pub fn build(self) -> Ready<Result<Scraper<B>, Error>> {
        ready(Ok(Scraper::from_parts(
            self.bmc,
            self.capacity,
            self.discovery,
        )))
    }
}
