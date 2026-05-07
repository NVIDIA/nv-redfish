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

//! Runtime control surface.

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use core::marker::PhantomData;
use std::error::Error as StdError;

use crate::generator::CostUnits;
use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;

/// Runtime-wide configuration set when the runtime is constructed.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Optional global maximum number of in-flight work items.
    pub global_max_in_flight: Option<u32>,
    /// Optional bound on the output queue. When `None` the queue is unbounded.
    pub output_queue_capacity: Option<usize>,
}

/// Per-target limits set when a target is added or updated.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetLimits {
    /// Maximum number of in-flight work items for this target.
    pub max_in_flight: Option<u32>,
    /// Maximum cost budget per scheduling round for this target.
    pub max_cost_per_round: Option<CostUnits>,
}

/// Per-generator configuration set when a generator is added or updated.
#[derive(Debug, Clone, Default)]
pub struct GeneratorConfig {
    /// Optional class identifier for class-based scheduling.
    pub class: Option<ClassId>,
    /// Optional service weight for weighted scheduling.
    pub weight: Option<u32>,
}

/// Errors returned when adding a generator fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AddGeneratorError {
    /// The target id does not exist (never added or already removed).
    TargetNotFound,
    /// Graceful shutdown has started; no new generators may be added.
    ShutdownStarted,
}

impl Display for AddGeneratorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::TargetNotFound => f.write_str("target not found"),
            Self::ShutdownStarted => f.write_str("graceful shutdown already started"),
        }
    }
}

impl StdError for AddGeneratorError {}

/// Cloneable handle to a running [`crate::Runtime`].
///
/// `RuntimeHandle` exposes the synchronous control surface. It can be cloned
/// and shared across tasks; mutating operations may briefly lock internal
/// state but never wait on work futures.
///
/// The runtime itself is *not* `Clone` — only one consumer drives the output
/// stream via `Runtime::next`.
pub struct RuntimeHandle<Ev, Err> {
    // Scaffold-only placeholder. Replaced with `Arc<Shared<Ev, Err>>` once the
    // runtime body is relocated into this crate.
    _phantom: PhantomData<fn() -> (Ev, Err)>,
}

impl<Ev, Err> Clone for RuntimeHandle<Ev, Err> {
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<Ev, Err> RuntimeHandle<Ev, Err> {
    /// Add a target to the runtime and return its newly-allocated id.
    ///
    /// If graceful shutdown has already started the call returns `None`.
    #[must_use]
    pub fn add_target(&self, _limits: TargetLimits) -> Option<TargetId> {
        unimplemented!("scaffold")
    }

    /// Remove the target with the given id.
    ///
    /// Returns `true` if the target existed. All attached generators are
    /// removed as part of this call.
    #[must_use]
    pub fn remove_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Update the limits of an existing target. Returns `true` on success.
    #[must_use]
    pub fn update_target_limits(&self, _id: TargetId, _limits: TargetLimits) -> bool {
        unimplemented!("scaffold")
    }

    /// Pause an existing target. Returns `true` on success.
    ///
    /// Pausing never wakes the parked runtime: a paused target only excludes
    /// itself from future selections, it does not produce any new ready
    /// work to consume.
    #[must_use]
    pub fn pause_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Resume a paused target. Returns `true` on success.
    #[must_use]
    pub fn resume_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Add a generator under the specified target.
    ///
    /// # Errors
    ///
    /// Returns [`AddGeneratorError::TargetNotFound`] if `target` is not registered.
    /// Returns [`AddGeneratorError::ShutdownStarted`] if graceful shutdown
    /// has begun.
    pub fn add_generator(
        &self,
        _target: TargetId,
        _generator: Box<dyn crate::Generator<Ev, Err> + Send>,
        _config: GeneratorConfig,
    ) -> Result<GeneratorId, AddGeneratorError> {
        unimplemented!("scaffold")
    }

    /// Remove a generator. Returns `true` if it existed.
    ///
    /// In-flight work for the removed generator continues to completion; only
    /// future selections are prevented.
    #[must_use]
    pub fn remove_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Update generator configuration. Returns `true` on success.
    #[must_use]
    pub fn update_generator(&self, _id: GeneratorId, _config: GeneratorConfig) -> bool {
        unimplemented!("scaffold")
    }

    /// Pause a generator. Returns `true` on success.
    ///
    /// Pausing never wakes the parked runtime; see [`Self::pause_target`].
    #[must_use]
    pub fn pause_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Resume a paused generator. Returns `true` on success.
    #[must_use]
    pub fn resume_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Hint to the scheduler that a generator should be considered ready now.
    ///
    /// Returns `true` if the generator exists.
    #[must_use]
    pub fn trigger_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Begin graceful shutdown. Idempotent: subsequent calls do nothing.
    ///
    /// After shutdown starts, mutating control operations reject new target
    /// and generator changes; in-flight work is allowed to complete; queued
    /// outputs are still delivered, and finally the sticky shutdown output is
    /// emitted by [`crate::Runtime::next`].
    pub fn graceful_shutdown(&self) {
        unimplemented!("scaffold")
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> crate::RuntimeStats {
        unimplemented!("scaffold")
    }
}
