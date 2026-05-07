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
//! `nv-redfish-dispatcher` is a Redfish-free, application-agnostic dispatcher
//! parameterized by a user work event type `Ev` and a work error type `Err`.
//! Its central abstraction is the [`Scheduler`] trait — every node in the
//! dispatcher's scheduling tree implements it, whether it is a *leaf* (a node
//! that produces work directly) or a *branch* (a node that composes children
//! using a scheduling policy: weighted DRR, round-robin, priority,
//! token-bucket admission, etc.).
//!
//! The runtime drives only the *root* node. Branches recurse internally, and
//! completions are forwarded back to the originating leaf via a per-work
//! [`RoutingPath`] breadcrumb.
//!
//! Public surface:
//!
//! - the [`Scheduler`] trait and its [`ScheduledWork`] / [`ScheduledWorkResult`] types,
//! - data types in [`work`]: [`WorkMeta`], [`Readiness`], [`CostUnits`],
//!   [`WorkCompletion`], [`CompletionOutcome`], [`RoutingPath`],
//! - opaque [`NodeId`] addressing every node in the tree,
//! - the single ordered output stream ([`RuntimeOutput`], [`WorkResult`], ...),
//! - optional out-of-band runtime events ([`RuntimeEventType`]),
//! - a cloneable synchronous control surface ([`RuntimeHandle`]),
//! - a single-consumer driver ([`Runtime`]) with [`Runtime::next`].
//!
//! This crate is currently a **scaffold**: the public types and signatures
//! are frozen, but bodies are stubbed with [`unimplemented!`]. Built-in
//! branch implementations (weighted DRR, round-robin, token bucket, etc.)
//! land in a follow-up phase.

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

#[doc(inline)]
pub use event::RuntimeEventType;
#[cfg(feature = "runtime-events")]
#[doc(inline)]
pub use event::RuntimeEvent;
#[doc(inline)]
pub use runtime::Runtime;
#[doc(inline)]
pub use scheduler::ScheduledWork;
#[doc(inline)]
pub use scheduler::ScheduledWorkResult;
#[doc(inline)]
pub use scheduler::Scheduler;
#[doc(inline)]
pub use stats::NodeStats;
#[doc(inline)]
pub use stats::RuntimeStats;
#[doc(inline)]
pub use stats::WorkStats;
#[doc(inline)]
pub use work::CompletionOutcome;
#[doc(inline)]
pub use work::CostUnits;
#[doc(inline)]
pub use work::Readiness;
#[doc(inline)]
pub use work::RoutingPath;
#[doc(inline)]
pub use work::WorkCompletion;
#[doc(inline)]
pub use work::WorkMeta;
