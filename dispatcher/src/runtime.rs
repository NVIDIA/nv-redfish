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

//! The generic dispatcher runtime.
//!
//! [`Runtime::next`] is the single ordered output and execution interface.
//! Each call advances the runtime by at most one selected work item, drains
//! at most one in-flight completion to the output queue, and returns the
//! oldest queued output. When nothing can make progress, the future parks
//! until a control-plane mutation or an in-flight task completes.

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

use crate::control::AddGeneratorError;
use crate::control::GeneratorConfig;
use crate::control::RuntimeConfig;
use crate::control::RuntimeHandle;
use crate::control::TargetLimits;
use crate::generator::Generator;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::output::RuntimeOutput;
use crate::stats::ClassStats;
use crate::stats::RuntimeStats;

/// Generic dispatcher runtime parameterized by application work event type
/// `Ev` and work error type `Err`.
///
/// The runtime is *not* `Clone`. Only one consumer drives the output stream
/// via [`Runtime::next`]. Use [`Runtime::handle`] to obtain cloneable control
/// handles for cross-task control operations.
pub struct Runtime<Ev, Err> {
    // Scaffold-only placeholder. Replaced with the real
    // `Arc<Shared<Ev, Err>>` + `FuturesUnordered` + bookkeeping fields when
    // the runtime body is relocated into this crate.
    _phantom: PhantomData<fn() -> (Ev, Err)>,
}

impl<Ev, Err> Runtime<Ev, Err>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    /// Build a new runtime with the given configuration.
    #[must_use]
    pub fn new(_config: RuntimeConfig) -> Self {
        unimplemented!("scaffold")
    }

    /// Return a cloneable handle that exposes the synchronous control surface.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err> {
        unimplemented!("scaffold")
    }

    /// Add a target to the runtime and return its newly-allocated id.
    ///
    /// Forwarded convenience for [`RuntimeHandle::add_target`].
    #[must_use]
    pub fn add_target(&self, _limits: TargetLimits) -> Option<TargetId> {
        unimplemented!("scaffold")
    }

    /// Remove a target. Forwarded convenience for [`RuntimeHandle::remove_target`].
    #[must_use]
    pub fn remove_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Update target limits. Forwarded convenience.
    #[must_use]
    pub fn update_target_limits(&self, _id: TargetId, _limits: TargetLimits) -> bool {
        unimplemented!("scaffold")
    }

    /// Pause a target. Forwarded convenience.
    #[must_use]
    pub fn pause_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Resume a target. Forwarded convenience.
    #[must_use]
    pub fn resume_target(&self, _id: TargetId) -> bool {
        unimplemented!("scaffold")
    }

    /// Add a generator. Forwarded convenience for [`RuntimeHandle::add_generator`].
    ///
    /// # Errors
    ///
    /// Forwarded from [`RuntimeHandle::add_generator`].
    pub fn add_generator(
        &self,
        _target: TargetId,
        _generator: Box<dyn Generator<Ev, Err> + Send>,
        _config: GeneratorConfig,
    ) -> Result<GeneratorId, AddGeneratorError> {
        unimplemented!("scaffold")
    }

    /// Remove a generator. Forwarded convenience.
    #[must_use]
    pub fn remove_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Update generator config. Forwarded convenience.
    #[must_use]
    pub fn update_generator(&self, _id: GeneratorId, _config: GeneratorConfig) -> bool {
        unimplemented!("scaffold")
    }

    /// Pause a generator. Forwarded convenience.
    #[must_use]
    pub fn pause_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Resume a generator. Forwarded convenience.
    #[must_use]
    pub fn resume_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Trigger a generator. Forwarded convenience.
    #[must_use]
    pub fn trigger_generator(&self, _id: GeneratorId) -> bool {
        unimplemented!("scaffold")
    }

    /// Begin graceful shutdown. Forwarded convenience.
    pub fn graceful_shutdown(&self) {
        unimplemented!("scaffold")
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        unimplemented!("scaffold")
    }

    /// Snapshot of per-class statistics.
    #[must_use]
    pub fn class_stats(&self) -> Vec<ClassStats> {
        unimplemented!("scaffold")
    }

    /// Drive the runtime by one step and return the next ordered output.
    ///
    /// Behavior summary:
    ///
    /// 1. If already-emitted shutdown is sticky, return it again.
    /// 2. Drain at most one already-queued output and return it.
    /// 3. Poll in-flight work; on completion enqueue the corresponding
    ///    [`RuntimeOutput::Work`] and call [`Generator::on_complete`] exactly
    ///    once.
    /// 4. If shutdown has started and there is no further in-flight work or
    ///    queued output, emit the sticky shutdown output.
    /// 5. Otherwise scan generators for the first ready one, call
    ///    `take_next`, and admit the future to the in-flight set.
    /// 6. If nothing can make progress, register the current waker and park.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> NextFuture<'_, Ev, Err> {
        NextFuture {
            runtime: self,
            _phantom: PhantomData,
        }
    }
}

/// Future returned by [`Runtime::next`]. See its docs for behavior.
pub struct NextFuture<'r, Ev, Err> {
    // Borrow the runtime exclusively to enforce the single-driver invariant.
    runtime: &'r mut Runtime<Ev, Err>,
    _phantom: PhantomData<fn() -> (Ev, Err)>,
}

impl<Ev, Err> Future for NextFuture<'_, Ev, Err>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    type Output = RuntimeOutput<Ev, Err>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Touch `runtime` so the field is considered used by the borrow checker
        // even before the real implementation lands.
        let _ = &self.runtime;
        unimplemented!("scaffold")
    }
}
