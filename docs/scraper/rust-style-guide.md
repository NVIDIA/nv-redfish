# Scraper Rust style guide

This guide is intentionally short. Read it before writing scraper code and during
review before declaring work complete.

## Scope and boundaries

- Keep the generic runtime independent of Redfish, `nv-redfish`, BMC transports,
  generated schema types, Carbide crates, HTTP clients, mocks, databases, and
  application models.
- Add only APIs, fields, modules, and configuration that are used by current
  behavior or tests.
- Keep application policy outside the runtime. Runtime code owns scheduling,
  execution, completion reporting, and ordered outputs only.
- Prefer typed APIs that make invalid states hard to express. Redfish adapter
  code should close over valid `nv-redfish` objects instead of using detached
  command languages.

## File headers

New Rust source and test files must start with this header:

```rust
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
```

New scraper `Cargo.toml` files must start with this header:

```toml
# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
```

## File and crate style

- Keep public modules documented and aligned with existing crate style.
- Scraper crate roots should use the same lint posture as `nv-redfish`:

  ```rust
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
      clippy::unimplemented,
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
  ```
- Use feature gates for optional capabilities. If a feature is disabled, its
  public builders, config fields, payload variants, and fetch code should not be
  compiled.
- Do not add placeholder feature flags, scheduler knobs, stats, costs, classes,
  limits, or runtime events before code or tests consume them.

## Import style

- Do not use grouped imports such as `use crate::{A, B};` or
  `use std::{collections::HashMap, time::Instant};` in scraper code.
- Prefer one imported item per `use` statement. This keeps diffs smaller and
  reduces merge conflicts.
- Keep imports local and explicit. Avoid broad wildcard imports except where a
  narrow test module has a clear reason.
- Apply this style to scraper source files, integration tests, and shared test
  helpers.

## Generic types and trait bounds

- Do not derive traits on public generic payload wrappers when that would impose
  unnecessary bounds on user types.
- Keep lifetime and trait bounds as narrow as possible. Do not force `'static`,
  `Clone`, `Debug`, `Send`, or `Sync` on payload types unless storage or async
  execution truly requires it.
- Prefer manual trait implementations when derive would require incorrect bounds
  on a generic parameter.
- Keep ids opaque. Expose only intentional accessors, such as recovering a parent
  target id from a generator id.

## Expression-oriented Rust

- Prefer direct expressions, `?`, `let else`, `map`, `and_then`, `transpose`,
  `is_some_and`, and `then_some` when they improve clarity.
- Avoid early `return` when a clear expression or `?` is simpler.
- Use `then_some(value)` instead of `then(|| value)` when the value is cheap and
  laziness is not needed.
- Do not use `Option::map`, `Result::map`, or iterator `map` only for side
  effects. Use `if let`, `for_each`, a direct loop, or restructure the code.
- Loops are acceptable when they make async sequencing, mutation boundaries, or
  error propagation clearer than an iterator pipeline.

## Iterators and collection style

- Prefer transforming data with iterator adapters and `collect()` rather than
  creating a mutable `Vec` and pushing into it.
- When a type annotation is needed for collection, prefer
  `collect::<Vec<_>>()` over a separate `let values: Vec<_> = ...collect()`.
- Keep output ordering deterministic. Runtime output queues and tests must
  preserve FIFO and per-work event order.
- Avoid timing-based assertions in tests.

## Mutation and state management

- Keep mutation localized at API or state boundaries.
- Prefer immutable planning followed by mutation. For example, compute ids to
  remove first, then remove them from all affected structures.
- Do not pass mutable out-parameters. Return computed values, updated cursor
  positions, updated state, or domain-specific result types.
- Avoid helper APIs that require callers to coordinate related mutable state such
  as scheduler cursors, remaining scan counts, or queue indices.
- Removed generators must not be queried again. Removed targets must remove all
  attached generators. Existing queued outputs should survive removal unless an
  explicit policy changes that behavior.

## Scheduler and generator style

- Model periodic flows as stateful generators, not queues of pre-created jobs.
- A generator reports readiness and creates executable work only when selected.
- Schedulers operate on abstract metadata and ids. They must not know Redfish,
  application semantics, payload shapes, or transport details.
- Keep scheduler data structures synchronous unless async scheduling is required.
- `run_once`-style execution should execute at most the selected work item and
  report completion exactly once.
- Enqueue work output before calling the generator completion callback unless an
  explicit policy changes that ordering.

## Error and documentation style

- Public async or fallible APIs should document `# Errors`.
- Error wrappers should carry the generic error value without requiring it to
  implement formatting traits unless needed.
- Prefer precise enum variants over stringly typed control flow.
- Use `#[must_use]` on value-returning helpers where ignoring the result would be
  surprising.

## Tests

- Runtime and scheduler tests should use fake generators, fake readiness, fake
  work, fake events, and fake errors. They should not require Redfish, HTTP, BMC
  mocks, generated schemas, or Carbide.
- Include at least one test shape that proves event and error payloads do not need
  `Clone`, `Debug`, `Eq`, or `PartialEq` when the public API should not require
  those bounds.
- Split integration tests by behavior domain, not by implementation phase. For
  example, prefer files such as `ids.rs`, `control.rs`, `scheduling.rs`,
  `output.rs`, `completion.rs`, and `discovery_flow.rs` over `phase_0.rs`.
- Put reusable fake generators, fake events, and assertion helpers in shared test
  helper modules when that avoids copy/paste without hiding test intent.
- Build reports or expected projections from drained outputs with iterator
  adapters when practical.
- Do not hide impossible configurations. Tests should make requested behavior and
  overload/failure outcomes explicit.

## Review checklist

- Does the implementation add only behavior currently required by code or tests?
- Are runtime boundaries still generic and application-policy-free?
- Are feature-gated capabilities absent from the build when disabled?
- Are generic payload wrappers free of unnecessary trait bounds?
- Are ids opaque and display/debug behavior intentional?
- Are outputs ordered and completions reported exactly as specified?
- Are removals complete and stale scheduler entries impossible?
- Are tests deterministic, fake-only where required, and checking requirements
  rather than implementation details?
- Did style drift introduce placeholder APIs, mutable out-parameters, stale job
  queues, or unnecessary loops/push-based collection?
