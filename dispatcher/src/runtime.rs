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
//! Each call drains the oldest queued output if one exists; otherwise it may
//! fill in-flight work up to [`RuntimeConfig::global_max_in_flight`], poll the
//! in-flight set, queue completed work outputs, and return once an output is
//! available. If no progress is possible, it parks.
//!
//! The runtime is policy-free and meta-blind. All scheduling lives in the
//! root [`Scheduler`] subtree; the runtime only dispatches payloads, polls
//! the in-flight set, forwards completions back through the tree, enforces
//! the runtime-wide caps in [`RuntimeConfig`], and emits [`RuntimeOutput`].
//!
//! The root sits behind an internal mutex. The driver and
//! [`RuntimeHandle::with_root`] / [`RuntimeHandle::with_root_mut`] share
//! the same lock, so user mutations and driver steps serialize naturally.

use crate::scheduler::private::SchedulerObj;
use crate::scheduler::Scheduler;
use crate::stats::RuntimeStats;
use crate::work::WorkMeta;
use crate::Completion;
use crate::CompletionOutcome;
use crate::RoutingPath;
use crate::RuntimeEventType;
use crate::ScheduledWork;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::time::Duration;
use futures_core::Stream as _;
use futures_util::stream::FuturesUnordered;
use std::collections::VecDeque;
use std::mem;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;
use std::time::Instant;

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
pub type FutureWork<Ev, Err> = Pin<Box<dyn Future<Output = Result<Vec<Ev>, Err>> + Send + 'static>>;

/// Generic dispatcher runtime, parameterized by event type `Ev`, error
/// type `Err`, and root meta type `M`.
///
/// `M` is whatever the root scheduler exposes as `Self::Meta` — typically
/// a stack of wrappers like `WithPriority<WithCost<()>>`.
///
/// Not `Clone`: only one consumer drives [`Runtime::next`]. Use
/// [`Runtime::handle`] for cloneable control handles.
pub struct Runtime<Ev, Err, M: WorkMeta> {
    config: RuntimeConfig,
    clock: RuntimeClock,
    in_flight: FuturesUnordered<InFlight<Ev, Err, M>>,
    completion: Vec<Completion<M>>,
    output: VecDeque<RuntimeOutput<Ev, Err>>,
    shared: Arc<Mutex<Shared<Ev, Err, M>>>,
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
    pub fn new<S>(config: RuntimeConfig, root: S) -> Self
    where
        S: Scheduler<FutureWork<Ev, Err>, Meta = M>,
    {
        Self {
            clock: match config.clock {
                ClockConfig::Wallclock => RuntimeClock::Wallclock,
                ClockConfig::Virtual(increment) => RuntimeClock::Virtual {
                    now: Instant::now(),
                    increment,
                },
            },
            in_flight: FuturesUnordered::new(),
            completion: Vec::new(),
            output: VecDeque::new(),
            config,
            shared: Mutex::new(Shared {
                root: Box::new(root),
                waker: None,
                _phantom: PhantomData,
            })
            .into(),
            _phantom: PhantomData,
        }
    }

    /// Cloneable handle for synchronous control and typed root access.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err, M> {
        RuntimeHandle {
            shared: self.shared.clone(),
        }
    }

    /// Advance until an output is available or no progress is possible.
    ///
    /// Step order:
    ///
    /// 1. Drain one queued output if any.
    /// 2. If below [`RuntimeConfig::global_max_in_flight`], lock shared
    ///    scheduler/control state briefly and dispatch available work until
    ///    the global cap is reached or the scheduler has no work.
    /// 3. If shared state has no work while capacity remains, register the
    ///    current waker so external control changes can wake this future.
    /// 4. Poll in-flight payloads without holding the shared lock; completed
    ///    payloads enqueue [`RuntimeOutput::Work`].
    /// 5. If no output was queued and no synchronous progress was made, park.
    ///
    /// Shares the root lock with [`RuntimeHandle::with_root_mut`]; both
    /// hold it briefly.
    pub const fn next(&mut self) -> NextFuture<'_, Ev, Err, M> {
        NextFuture { runtime: self }
    }
}

/// Future returned by [`Runtime::next`].
pub struct NextFuture<'r, Ev, Err, M: WorkMeta> {
    // Exclusive borrow enforces the single-driver invariant.
    runtime: &'r mut Runtime<Ev, Err, M>,
}

impl<Ev, Err, M> Future for NextFuture<'_, Ev, Err, M>
where
    Ev: Send + 'static,
    Err: Send + 'static,
    M: WorkMeta,
{
    type Output = RuntimeOutput<Ev, Err>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut progress = true;
        while progress {
            if !self.runtime.completion.is_empty() {
                let completions = mem::take(&mut self.runtime.completion);
                let mut shared = self
                    .runtime
                    .shared
                    .lock()
                    .expect("dispatcher runtime mutex is poisoned");
                for completion in completions {
                    shared.root.on_complete(completion);
                }
            }
            if let Some(output) = self.runtime.output.pop_front() {
                return Poll::Ready(output);
            }
            progress = false;

            let now = self.runtime.clock.now();
            let mut in_flight = mem::take(&mut self.runtime.in_flight);
            let global_max_in_flight = self.runtime.config.global_max_in_flight;
            if in_flight.len() < global_max_in_flight.into() {
                let mut shared = self
                    .runtime
                    .shared
                    .lock()
                    .expect("dispatcher runtime mutex is poisoned");
                while in_flight.len() < global_max_in_flight.into() {
                    match shared.next(now) {
                        SharedNextResult::Work(work) => {
                            in_flight.push(InFlight {
                                start: now,
                                work: Some(work),
                            });
                        }
                        SharedNextResult::SleepUntil(v) => {
                            if in_flight.is_empty() {
                                return Poll::Ready(RuntimeOutput::SleepUntil(v));
                            }
                            break;
                        }
                        SharedNextResult::Nothing => {
                            break;
                        }
                    }
                }
                // Setup waker if change in shared elements can cause
                // progress for the future.
                if in_flight.len() < global_max_in_flight.into() {
                    shared.maybe_setup_waker(cx);
                }
            }

            while let Poll::Ready(Some(completed)) = Pin::new(&mut in_flight).poll_next(cx) {
                let CompletedWork {
                    start,
                    meta,
                    result,
                    routing,
                } = completed;
                let latency = now.duration_since(start);
                progress = true;
                self.runtime.completion.push(Completion {
                    latency,
                    meta,
                    routing,
                    outcome: if result.is_ok() {
                        CompletionOutcome::Succeeded
                    } else {
                        CompletionOutcome::Failed
                    },
                });
                self.runtime
                    .output
                    .push_back(RuntimeOutput::Work { result, latency });
            }
            self.runtime.in_flight = in_flight;
        }
        Poll::Pending
    }
}

/// Runtime-wide configuration. Per-node policy lives inside each
/// [`Scheduler`]; this struct only carries knobs no node owns.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig {
    /// Global cap on in-flight items, applied at dispatch admission
    /// on top of any per-subtree admission a branch enforces.
    pub global_max_in_flight: NonZeroUsize,
    /// Runtime clock configuration.
    pub clock: ClockConfig,
}

/// Runtime clock configuration.
#[derive(Clone, Copy, Debug, Default)]
pub enum ClockConfig {
    /// Clock that ticks with real time.
    #[default]
    Wallclock,
    /// Clock that increments on specified duration each time it is
    /// requested.
    Virtual(Duration),
}

/// Cloneable handle to a running [`Runtime`].
///
/// Exposes synchronous control plus typed root access. Mutating ops take
/// the internal lock briefly and never wait on work payloads. The runtime
/// itself is not `Clone`.
pub struct RuntimeHandle<Ev, Err, M: WorkMeta> {
    shared: Arc<Mutex<Shared<Ev, Err, M>>>,
}

impl<Ev, Err, M: WorkMeta> Clone for RuntimeHandle<Ev, Err, M> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
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
    ///
    /// # Panics
    ///
    /// Can panic if runtime mutex is poisoned. Which only can happen
    /// if any f passed to this function paniced.
    #[allow(clippy::unwrap_in_result)]
    pub fn with_root_mut<S, R>(&self, f: impl FnOnce(&mut S) -> R) -> Option<R>
    where
        S: 'static,
    {
        let mut guard = self
            .shared
            .lock()
            .expect("dispatcher runtime mutex is poisoned");
        let result = guard.root.as_any_mut().downcast_mut::<S>().map(f);
        let waker = guard.waker.take();
        drop(guard);
        if let Some(waker) = waker {
            waker.wake();
        }
        result
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
    /// Runtime requested to sleep specified duration. In tokio it is
    /// expected that caller will call
    /// tokio::time::sleep(v.duration_since(now)).await before calling
    /// next() next time.
    SleepUntil(Instant),
    /// Sticky terminal output after graceful shutdown drains. Subsequent
    /// `next()` calls return this immediately.
    Shutdown,
}

enum RuntimeClock {
    Wallclock,
    Virtual { now: Instant, increment: Duration },
}

impl RuntimeClock {
    fn now(&mut self) -> Instant {
        match self {
            Self::Wallclock => Instant::now(),
            Self::Virtual { now, increment } => {
                *now += *increment;
                *now
            }
        }
    }
}

struct Shared<Ev, Err, M> {
    waker: Option<Waker>,
    root: Box<dyn SchedulerObj<FutureWork<Ev, Err>, M>>,
    _phantom: PhantomData<(Ev, Err)>,
}

impl<Ev, Err, M> Shared<Ev, Err, M>
where
    Ev: 'static,
    Err: 'static,
    M: Send + 'static,
{
    fn next(&mut self, now: Instant) -> SharedNextResult<Ev, Err, M> {
        let r = self.root.update_ready(now);
        if r.ready {
            if let Some(work) = self.root.take_next() {
                return SharedNextResult::Work(work);
            }
        }
        r.next_update_at
            .map_or(SharedNextResult::Nothing, SharedNextResult::SleepUntil)
    }

    fn maybe_setup_waker(&mut self, cx: &Context<'_>) {
        let waker = cx.waker();

        if self
            .waker
            .as_ref()
            .is_none_or(|old_waker| !old_waker.will_wake(waker))
        {
            self.waker = Some(waker.clone());
        }
    }
}

enum SharedNextResult<Ev, Err, M>
where
    Ev: 'static,
    Err: 'static,
    M: Send + 'static,
{
    Work(ScheduledWork<FutureWork<Ev, Err>, M>),
    SleepUntil(Instant),
    Nothing,
}

struct InFlight<Ev, Err, M: WorkMeta> {
    start: Instant,
    work: Option<ScheduledWork<FutureWork<Ev, Err>, M>>,
}

impl<Ev, Err, M: WorkMeta> Unpin for InFlight<Ev, Err, M> {}

impl<Ev, Err, M: WorkMeta> Future for InFlight<Ev, Err, M> {
    type Output = CompletedWork<Ev, Err, M>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut work = this
            .work
            .take()
            .expect("in-flight work polled after completion");
        match work.payload.as_mut().poll(cx) {
            Poll::Pending => {
                this.work = Some(work);
                Poll::Pending
            }
            Poll::Ready(result) => Poll::Ready(CompletedWork {
                start: this.start,
                result,
                meta: work.meta,
                routing: work.routing,
            }),
        }
    }
}

struct CompletedWork<Ev, Err, M> {
    start: Instant,
    meta: M,
    result: Result<Vec<Ev>, Err>,
    routing: RoutingPath,
}
