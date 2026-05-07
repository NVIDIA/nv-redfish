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
//!
//! The runtime is policy-free: every scheduling decision lives inside the
//! installed root [`Scheduler`] subtree. The runtime is responsible for:
//!
//! - dispatching work futures returned by `root.take_next()`,
//! - polling the in-flight set,
//! - delivering completions back through `root.on_complete(&mut completion)`
//!   (branches recurse via [`crate::RoutingPath`]),
//! - enforcing only runtime-wide invariants ([`RuntimeConfig::global_max_in_flight`],
//!   [`RuntimeConfig::output_queue_capacity`]),
//! - producing the single ordered [`crate::RuntimeOutput`] stream.

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

use crate::RuntimeEventType;
use crate::scheduler::Scheduler;
use crate::stats::RuntimeStats;
use crate::work::WorkResult;

/// Generic dispatcher runtime parameterized by application work event type
/// `Ev` and work error type `Err`.
///
/// The runtime is *not* `Clone`. Only one consumer drives the output stream
/// via [`Runtime::next`]. Use [`Runtime::handle`] to obtain cloneable control
/// handles for cross-task control operations.
pub struct Runtime<Ev, Err> {
    // Scaffold-only placeholder. Replaced with real
    // `Arc<Shared<Ev, Err>>` + `FuturesUnordered` + bookkeeping fields when
    // the runtime body lands.
    _phantom: PhantomData<fn() -> (Ev, Err)>,
}

impl<Ev, Err> Runtime<Ev, Err>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    /// Build a new runtime with the given configuration.
    #[must_use]
    pub fn new(_config: RuntimeConfig, _root: Box<dyn Scheduler<Ev, Err> + Send>) -> Self {
        unimplemented!("scaffold")
    }

    /// Return a cloneable handle that exposes the synchronous control surface.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err> {
        unimplemented!("scaffold")
    }

    /// Drive the runtime by one step and return the next ordered output.
    ///
    /// Behavior summary:
    ///
    /// 1. If a sticky shutdown has been emitted, return it again.
    /// 2. Drain at most one already-queued output and return it.
    /// 3. Poll in-flight work; on completion enqueue the corresponding
    ///    [`crate::RuntimeOutput::Work`] and call
    ///    [`Scheduler::on_complete`] on the root exactly once. Branches
    ///    propagate the call to the originating child by popping their
    ///    [`crate::RoutingPath`] tag.
    /// 4. If shutdown has started and there is no further in-flight work or
    ///    queued output, emit the sticky shutdown output.
    /// 5. Otherwise call `root.update_ready(now)`; if ready, call
    ///    `root.take_next()` and admit the resulting future to the in-flight
    ///    set, subject to [`RuntimeConfig::global_max_in_flight`].
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
        let _ = &self.runtime;
        unimplemented!("scaffold")
    }
}


/// Runtime-wide configuration set when the runtime is constructed.
///
/// Per-node policy lives inside each [`Scheduler`] implementation; this
/// struct is intentionally minimal and only carries runtime-wide knobs that
/// no individual node owns.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Optional global maximum number of in-flight work items.
    ///
    /// Enforced by the runtime on dispatch admission, in addition to whatever
    /// per-subtree admission a branch node may impose.
    pub global_max_in_flight: Option<u32>,
    /// Optional bound on the output queue. When `None` the queue is unbounded.
    pub output_queue_capacity: Option<usize>,
}

/// Cloneable handle to a running [`crate::Runtime`].
///
/// `RuntimeHandle` exposes the synchronous control surface. It can be cloned
/// and shared across tasks; mutating operations may briefly lock internal
/// state but never wait on work futures.
///
/// The runtime itself is *not* `Clone` — only one consumer drives the output
/// stream via [`crate::Runtime::next`].
pub struct RuntimeHandle<Ev, Err> {
    // Scaffold-only placeholder. Replaced with `Arc<Shared<Ev, Err>>` once the
    // runtime body is filled in.
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
    /// Begin graceful shutdown. Idempotent: subsequent calls do nothing.
    ///
    /// After shutdown starts, mutating control operations reject new node
    /// installations; in-flight work is allowed to complete; queued outputs
    /// are still delivered, and finally the sticky shutdown output is
    /// emitted by [`crate::Runtime::next`].
    pub fn graceful_shutdown(&self) {
        unimplemented!("scaffold")
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        unimplemented!("scaffold")
    }
}

/// Single ordered output value emitted by the runtime.
///
/// `R` defaults to [`crate::RuntimeEventType`] which is
/// [`core::convert::Infallible`] when the `runtime-events` feature is
/// disabled, making `RuntimeOutput::Runtime(_)` impossible to construct.
pub enum RuntimeOutput<Ev, Err, R = RuntimeEventType> {
    /// Application or adapter work output.
    Work(WorkResult<Ev, Err>),
    /// Out-of-band runtime event. Only constructible when `runtime-events`
    /// is enabled (otherwise `R = Infallible`).
    Runtime(R),
    /// Sticky terminal output emitted after graceful shutdown drains in-flight
    /// work and prior queued output. Subsequent `next()` calls return this
    /// variant immediately.
    Shutdown,
}
