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

//! Generic cooperative task dispatcher with composable scheduling.
//!
//! Every node in the scheduling tree implements one trait, [`Scheduler`].
//! Leaves produce work; branches compose children with a policy (DRR,
//! round-robin, priority, token bucket, …). The runtime drives only the
//! *root*; branches recurse, and completions flow back via a per-work
//! [`RoutingPath`] breadcrumb.
//!
//! [`Scheduler<T>`] is parameterized only by an opaque payload `T`. The
//! scheduler tree never inspects it; the runtime does. This crate's
//! [`Runtime`] uses `T = FutureWork<Ev, Err>` (boxed futures returning
//! `Result<Vec<Ev>, Err>`); other runtimes can pick another shape and
//! reuse the same scheduler types.
//!
//! ## Layered metadata
//!
//! [`WorkMeta`] is a marker bound (`Debug + Clone + Send + 'static`) — any
//! matching type is meta, and `()` is the canonical no-policy meta. Policy
//! data is added by *wrappers* ([`WithCost`], [`WithPriority`]) that
//! implement *projection traits* ([`HasCost`], [`HasPriority`]); schedulers
//! ask for the projections they need and leave the rest alone.
//! [`RoutingPath`] is a structural sibling of meta on [`ScheduledWork`] and
//! [`Completion`], not a meta concern.
//!
//! ## Public surface
//!
//! - [`Scheduler`], [`ScheduledWork`], [`Completion`],
//! - [`WorkMeta`], wrappers [`WithCost`] / [`WithPriority`], projections
//!   [`HasCost`] / [`HasPriority`],
//! - [`Readiness`], [`CostUnits`], [`CompletionOutcome`], [`RoutingPath`],
//! - [`Runtime`] + [`RuntimeConfig`], [`RuntimeHandle`] (with
//!   [`RuntimeHandle::with_root`] / [`with_root_mut`][`RuntimeHandle::with_root_mut`]),
//!   [`RuntimeOutput`], the [`FutureWork`] payload alias,
//! - optional out-of-band [`RuntimeEventType`].
//!
//! The runtime does *not* enumerate the scheduler tree, and exposes no
//! per-leaf identity or telemetry. Schedulers that need such observability
//! expose it through their own API, reached via [`RuntimeHandle::with_root`].
//!
//! This crate is currently a **scaffold**: signatures are frozen, bodies
//! are stubbed with [`unimplemented!`]. Built-in branch policies land in a
//! follow-up phase.

#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![deny(missing_docs)]
#![allow(clippy::doc_markdown)]
// Module-name repetition is intentional for this crate's public types
// (RuntimeOutput, RuntimeEvent, RuntimeStats, etc.) which are re-exported.
#![allow(clippy::module_name_repetitions)]
// Scaffold-only relaxations. Removed when the implementations land.
#![allow(clippy::unimplemented)]
#![allow(dead_code)]

pub mod event;
pub mod runtime;
pub mod scheduler;
pub mod stats;
pub mod work;

#[cfg(feature = "runtime-events")]
#[doc(inline)]
pub use event::RuntimeEvent;
#[doc(inline)]
pub use event::RuntimeEventType;
#[doc(inline)]
pub use runtime::FutureWork;
#[doc(inline)]
pub use runtime::Runtime;
#[doc(inline)]
pub use runtime::RuntimeConfig;
#[doc(inline)]
pub use runtime::RuntimeHandle;
#[doc(inline)]
pub use runtime::RuntimeOutput;
#[doc(inline)]
pub use scheduler::ScheduledWork;
#[doc(inline)]
pub use scheduler::Scheduler;
#[doc(inline)]
pub use stats::OutputQueueStats;
#[doc(inline)]
pub use stats::RuntimeStats;
#[doc(inline)]
pub use work::Completion;
#[doc(inline)]
pub use work::CompletionOutcome;
#[doc(inline)]
pub use work::CostUnits;
#[doc(inline)]
pub use work::HasCost;
#[doc(inline)]
pub use work::HasPriority;
#[doc(inline)]
pub use work::Readiness;
#[doc(inline)]
pub use work::RoutingPath;
#[doc(inline)]
pub use work::WithCost;
#[doc(inline)]
pub use work::WithPriority;
#[doc(inline)]
pub use work::WorkMeta;
