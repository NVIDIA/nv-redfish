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

//! Generic dispatcher runtime.
//!
//! [`Runtime::next`] is the single ordered execution and output interface.
//! Each call advances by at most one selected item, drains at most one
//! completion, and returns the oldest queued output; otherwise it parks.
//!
//! The runtime is policy-free and meta-blind. All scheduling lives in the
//! root [`Scheduler`] subtree; the runtime only dispatches payloads, polls
//! the in-flight set, forwards completions back through the tree, enforces
//! the runtime-wide caps in [`RuntimeConfig`], and emits [`RuntimeOutput`].
//!
//! The root sits behind an internal mutex. The driver and
//! [`RuntimeHandle::with_root`] / [`RuntimeHandle::with_root_mut`] share
//! the same lock, so user mutations and driver steps serialize naturally.

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::time::Duration;

use crate::RuntimeEventType;
use crate::scheduler::Scheduler;
use crate::stats::RuntimeStats;
use crate::work::WorkMeta;

/// `PhantomData` alias for the runtime's three type parameters; factored
/// to keep struct types simple under `clippy::type_complexity`.
type RuntimePhantom<Ev, Err, M> = PhantomData<fn() -> (Ev, Err, M)>;

/// Work payload consumed by this runtime: a boxed future with terminal
/// value `Result<Vec<Ev>, Err>`. Schedulers parameterized as
/// `Scheduler<FutureWork<Ev, Err>>` are compatible with [`Runtime::new`].
///
/// [`crate::Scheduler`] is generic over the payload and never inspects it,
/// so alternate runtimes can pick another shape (sync closures, batched
/// descriptors, …) and reuse the same scheduler types.
pub type FutureWork<Ev, Err> =
    Pin<Box<dyn Future<Output = Result<Vec<Ev>, Err>> + Send + 'static>>;

/// Generic dispatcher runtime, parameterized by event type `Ev`, error
/// type `Err`, and root meta type `M`.
///
/// `M` is whatever the root scheduler exposes as `Self::Meta` — typically
/// a stack of wrappers like `WithPriority<WithCost<()>>`.
///
/// Not `Clone`: only one consumer drives [`Runtime::next`]. Use
/// [`Runtime::handle`] for cloneable control handles.
pub struct Runtime<Ev, Err, M: WorkMeta> {
    // Scaffold placeholder. Becomes
    // `Arc<Shared<Ev, Err, M>>` with a mutex-guarded
    // `Box<dyn private::SchedulerObj<FutureWork<Ev, Err>, M> + Send>`,
    // a `FuturesUnordered`, and bookkeeping.
    _phantom: RuntimePhantom<Ev, Err, M>,
}

impl<Ev, Err, M> Runtime<Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    /// Build a runtime with the given configuration and root scheduler.
    ///
    /// The bound `S: Scheduler<FutureWork<Ev, Err>, Meta = M>` ties the
    /// tree's payload to the shape this runtime executes. The root is
    /// consumed and stored behind a mutex; reach into it later with
    /// [`RuntimeHandle::with_root`] / [`RuntimeHandle::with_root_mut`] by
    /// supplying the same concrete type for the downcast.
    ///
    /// A blanket `impl Scheduler for Box<S>` lets you pass an existing
    /// `Box<dyn Scheduler<FutureWork<Ev, Err>, Meta = M>>` directly.
    #[must_use]
    pub fn new<S>(_config: RuntimeConfig, _root: S) -> Self
    where
        S: Scheduler<FutureWork<Ev, Err>, Meta = M>,
    {
        unimplemented!("scaffold")
    }

    /// Cloneable handle for synchronous control and typed root access.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err, M> {
        unimplemented!("scaffold")
    }

    /// Advance one step and return the next output.
    ///
    /// Step order:
    ///
    /// 1. If shutdown was already emitted, return it again.
    /// 2. Drain one queued output if any.
    /// 3. Poll in-flight payloads; enqueue [`RuntimeOutput::Work`] on
    ///    completion and call `root.on_complete` exactly once (branches
    ///    recurse via [`crate::RoutingPath`]).
    /// 4. After shutdown, once nothing remains, emit the sticky shutdown.
    /// 5. Otherwise refresh readiness and dispatch one item, subject to
    ///    [`RuntimeConfig::global_max_in_flight`].
    /// 6. Park otherwise.
    ///
    /// Shares the root lock with [`RuntimeHandle::with_root_mut`]; both
    /// hold it briefly.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> NextFuture<'_, Ev, Err, M> {
        NextFuture {
            runtime: self,
            _phantom: PhantomData,
        }
    }
}

/// Future returned by [`Runtime::next`].
pub struct NextFuture<'r, Ev, Err, M: WorkMeta> {
    // Exclusive borrow enforces the single-driver invariant.
    runtime: &'r mut Runtime<Ev, Err, M>,
    _phantom: RuntimePhantom<Ev, Err, M>,
}

impl<Ev, Err, M> Future for NextFuture<'_, Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    type Output = RuntimeOutput<Ev, Err>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _ = &self.runtime;
        unimplemented!("scaffold")
    }
}

/// Runtime-wide configuration. Per-node policy lives inside each
/// [`Scheduler`]; this struct only carries knobs no node owns.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Optional global cap on in-flight items, applied at dispatch
    /// admission on top of any per-subtree admission a branch enforces.
    pub global_max_in_flight: Option<u32>,
    /// Optional output queue bound. `None` means unbounded.
    pub output_queue_capacity: Option<usize>,
}

/// Cloneable handle to a running [`Runtime`].
///
/// Exposes synchronous control plus typed root access. Mutating ops take
/// the internal lock briefly and never wait on work payloads. The runtime
/// itself is not `Clone`.
pub struct RuntimeHandle<Ev, Err, M: WorkMeta> {
    // Scaffold placeholder; becomes `Arc<Shared<Ev, Err, M>>`.
    _phantom: RuntimePhantom<Ev, Err, M>,
}

impl<Ev, Err, M: WorkMeta> Clone for RuntimeHandle<Ev, Err, M> {
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<Ev, Err, M> RuntimeHandle<Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    /// Begin graceful shutdown. Idempotent. In-flight items still complete,
    /// queued outputs still drain, then [`Runtime::next`] emits a sticky
    /// shutdown.
    pub fn graceful_shutdown(&self) {
        unimplemented!("scaffold")
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        unimplemented!("scaffold")
    }

    /// Run `f` with shared access to the root downcast to `S`. `None` if
    /// the downcast fails.
    ///
    /// Holds the root lock for the duration of `f`; keep it short and do
    /// not re-enter the runtime from inside.
    pub fn with_root<S, R>(&self, _f: impl FnOnce(&S) -> R) -> Option<R>
    where
        S: 'static,
    {
        unimplemented!("scaffold")
    }

    /// Run `f` with exclusive access to the root downcast to `S`. `None`
    /// if the downcast fails.
    ///
    /// Holds the root lock for the duration of `f`; keep it short and do
    /// not re-enter the runtime from inside (it will deadlock).
    pub fn with_root_mut<S, R>(&self, _f: impl FnOnce(&mut S) -> R) -> Option<R>
    where
        S: 'static,
    {
        unimplemented!("scaffold")
    }
}

/// Single ordered output emitted by the runtime.
///
/// `R` defaults to [`crate::RuntimeEventType`], which is
/// [`core::convert::Infallible`] when the `runtime-events` feature is off
/// — `RuntimeOutput::Runtime(_)` is then unconstructible.
pub enum RuntimeOutput<Ev, Err, R = RuntimeEventType> {
    /// Terminal value of one work payload, plus its wall-clock latency.
    Work {
        /// `Ok(events)` (one or more events in order) or `Err(error)`.
        result: Result<Vec<Ev>, Err>,
        /// Latency between dispatch and completion.
        latency: Duration,
    },
    /// Out-of-band runtime event (only when `runtime-events` is enabled).
    Runtime(R),
    /// Sticky terminal output after graceful shutdown drains. Subsequent
    /// `next()` calls return this immediately.
    Shutdown,
}
