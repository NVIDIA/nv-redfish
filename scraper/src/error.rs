// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;

use crate::QueryId;

/// Scraper error.
#[derive(Clone, Debug)]
pub enum Error {
    /// BMC operation failed.
    Bmc(Arc<dyn StdError + Send + Sync>),
    /// Discovery operation failed.
    Discovery(DiscoveryError),
    /// Query operation failed.
    Query(QueryError),
    /// Scheduler operation failed.
    Scheduler(SchedulerError),
    /// Store operation failed.
    Store(StoreError),
}

/// Discovery subsystem error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryError {
    /// Discovery context was missing scheduler-routed raw fetch support.
    RawFetchUnavailable,
}

/// Query subsystem error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    /// Query state lock was poisoned.
    LockPoisoned(String),
    /// Query event stream was closed.
    EventStreamClosed,
    /// Query event stream lagged behind the broadcast buffer.
    EventStreamLagged(u64),
    /// Query plan no longer exists.
    UnknownQuery(QueryId),
}

/// Scheduler subsystem error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    /// Scheduler state lock was poisoned.
    LockPoisoned(String),
    /// Scheduler admission channel closed.
    AdmissionClosed(String),
    /// Coalesced refresh owner was cancelled before publishing a result.
    CoalescedOwnerCancelled,
}

/// Store subsystem error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// Store state lock was poisoned.
    LockPoisoned(String),
    /// Coalesced refresh value had a different type than the waiting caller.
    CoalescedRefreshTypeMismatch,
}

impl Error {
    pub(crate) fn bmc(error: impl StdError + Send + Sync + 'static) -> Self {
        Self::Bmc(Arc::new(error))
    }

    pub(crate) fn query_lock(error: impl Display) -> Self {
        Self::Query(QueryError::LockPoisoned(error.to_string()))
    }

    pub(crate) fn scheduler_lock(error: impl Display) -> Self {
        Self::Scheduler(SchedulerError::LockPoisoned(error.to_string()))
    }

    pub(crate) fn store_lock(error: impl Display) -> Self {
        Self::Store(StoreError::LockPoisoned(error.to_string()))
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Bmc(error) => write!(formatter, "bmc: {error}"),
            Self::Discovery(error) => write!(formatter, "discovery: {error}"),
            Self::Query(error) => write!(formatter, "query: {error}"),
            Self::Scheduler(error) => write!(formatter, "scheduler: {error}"),
            Self::Store(error) => write!(formatter, "store: {error}"),
        }
    }
}

impl StdError for Error {}

impl Display for DiscoveryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::RawFetchUnavailable => formatter.write_str("raw fetch is unavailable"),
        }
    }
}

impl Display for QueryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::LockPoisoned(error) => write!(formatter, "{error}"),
            Self::EventStreamClosed => formatter.write_str("subscription event stream closed"),
            Self::EventStreamLagged(count) => {
                write!(formatter, "subscription event stream lagged by {count}")
            }
            Self::UnknownQuery(id) => write!(formatter, "unknown query {}", id.as_u64()),
        }
    }
}

impl Display for SchedulerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::LockPoisoned(error) | Self::AdmissionClosed(error) => {
                write!(formatter, "{error}")
            }
            Self::CoalescedOwnerCancelled => {
                formatter.write_str("coalesced refresh owner was cancelled")
            }
        }
    }
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::LockPoisoned(error) => write!(formatter, "{error}"),
            Self::CoalescedRefreshTypeMismatch => {
                formatter.write_str("coalesced refresh type mismatch")
            }
        }
    }
}
