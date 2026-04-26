# Scraper phase 0: generic runtime MVP

Phase 0 builds the first usable slice of the scraper product: a generic runtime
with the intended control, generator, execution, and output model. The runtime is
not Redfish-specific and must be testable without Carbide, Redfish, HTTP, BMC
mocks, or generated schema types.

The implementation is intentionally small. It includes target identity and a
final-ish work/output shape, but internally uses one flat round-robin scheduler
across all generators. Target limits, hierarchical scheduling, costs, budgets,
classes, and Redfish adapter work are later phases.

## Product goal

Create a new workspace library crate named `nv-redfish-scraper` at:

```text
scraper/
```

Phase 0 must provide a generic runtime that can:

- create targets with runtime-generated target ids,
- add generators under targets,
- remove generators,
- remove targets and all their generators,
- ask generators for readiness,
- select ready generators in round-robin order,
- execute one selected async work item per runtime step,
- publish ordered work results,
- report completion back to the originating generator,
- support fallible work through a generic error type,
- support a BMC-explorer-like discovery-flow test with fake data.

The runtime must not know what a BMC, Redfish service root, chassis, system,
endpoint report, database row, or application model is.

## Crate placement

Add a new workspace crate:

```text
scraper/
  Cargo.toml
  src/
    lib.rs
    generator.rs
    ids.rs
    output.rs
    runtime.rs
    scheduler/
      mod.rs
      flat_rr.rs
```

The package name must be:

```toml
name = "nv-redfish-scraper"
```

Add `scraper` to the workspace members in the root `Cargo.toml` when building
this phase in the `nv-redfish` workspace.

Phase 0 implementation should use only generic Rust/runtime concepts. It must
not depend on `nv-redfish`, `nv-redfish-core`, generated Redfish schemas,
Carbide crates, HTTP transports, BMC mocks, or database crates.

Tests may use an async test executor such as Tokio if the workspace already uses
it. The runtime implementation itself should keep as much logic synchronous as
possible and bind to async only at scheduled-work execution.

## Non-goals

Do not implement these in Phase 0:

- Redfish adapter generators,
- BMC/client construction,
- target limits,
- global limits,
- root scheduler,
- per-target scheduler,
- costs or `CostUnits`,
- classes or weights,
- WRR or DRR,
- budgets or rate limiting,
- concurrent/background executor,
- runtime driver task,
- pause/resume APIs,
- `run_until_idle`,
- stream subscription API,
- runtime event emission,
- serialization features,
- persistence,
- application discovery policy.

If a field, function, or module is not used by Phase 0 behavior, do not add it.
Prefer a small API over placeholder implementation.

## Runtime responsibilities

Phase 0 runtime owns four responsibilities:

1. control plane: targets and generators,
2. scheduling: choose which generator may create work,
3. execution: await one selected `ScheduledWork`,
4. output: enqueue ordered `RuntimeOutput` values.

The runtime is not merely a scheduler wrapper. The scheduler is an internal,
synchronous data structure used by the runtime.

## Public API overview

The exact module layout may be adjusted for idiomatic Rust, but Phase 0 should
expose these concepts from the crate root so tests and applications can use the
runtime without depending on private modules:

```rust
pub struct Runtime<E, Err, R = std::convert::Infallible>;

pub struct TargetConfig {}

pub struct TargetId { /* private */ }
pub struct GeneratorId { /* private */ }

pub enum AddGeneratorError {
    TargetNotFound { target_id: TargetId },
}

pub struct Readiness {
    pub ready: bool,
    pub next_ready_at: Option<std::time::Instant>,
}

pub trait Generator<E, Err>: Send {
    fn update_ready(&mut self, now: std::time::Instant) -> Readiness;
    fn take_next(&mut self) -> Option<ScheduledWork<E, Err>>;
    fn on_complete(&mut self, completion: &WorkCompletion);
}

pub struct ScheduledWork<E, Err> { /* private */ }

impl<E, Err> ScheduledWork<E, Err> {
    pub fn new<F>(future: F) -> Self
    where
        F: std::future::Future<Output = Result<Vec<E>, Err>> + Send + 'static;
}

pub type WorkResult<E, Err> = Result<WorkSuccess<E>, WorkError<Err>>;

pub struct WorkSuccess<E> {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub events: Vec<E>,
}

pub struct WorkError<Err> {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub error: Err,
}

pub enum RuntimeOutput<E, Err, R = std::convert::Infallible> {
    Work(WorkResult<E, Err>),
    Runtime(R),
}

pub struct WorkCompletion {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub outcome: WorkOutcome,
}

pub enum WorkOutcome {
    Succeeded,
    Failed,
}

pub enum RunOnce {
    Executed,
    Idle,
}

impl<E, Err, R> Runtime<E, Err, R> {
    pub fn new() -> Self;

    pub fn add_target(&mut self, config: TargetConfig) -> TargetId;
    pub fn remove_target(&mut self, target_id: TargetId) -> bool;

    pub fn add_generator<G>(
        &mut self,
        target_id: TargetId,
        generator: G,
    ) -> Result<GeneratorId, AddGeneratorError>
    where
        G: Generator<E, Err> + 'static;

    pub fn remove_generator(&mut self, generator_id: GeneratorId) -> bool;

    pub async fn run_once(&mut self) -> RunOnce;

    pub fn next_output(&mut self) -> Option<RuntimeOutput<E, Err, R>>;
    pub fn drain_outputs(&mut self) -> Vec<RuntimeOutput<E, Err, R>>;
}
```


- backed internally by a monotonic `u64`,
- allocation starts at `1`,
- removed ids are never reused,
- raw numeric internals are private,
- implements `Clone`, `Copy`, `Debug`, `Eq`, `PartialEq`, and `Hash`,
- implements `Display` as `target #N`.

Example display string:

```text
target #1
```

Future:

- target ids remain opaque,
- later target scheduler state and limits attach to `TargetId`,
- application-owned BMC/IP identity remains outside the runtime.

### `GeneratorId`

`GeneratorId` identifies one generator under one target. It must contain the full
identity: parent target id plus per-target generator sequence.

MVP behavior:

- allocated by the runtime when `add_generator` succeeds,
- generator sequence starts at `1` per target,
- removed ids are never reused,
- raw numeric internals are private,
- exposes `target_id()` to recover its parent `TargetId`,
- implements `Clone`, `Copy`, `Debug`, `Eq`, `PartialEq`, and `Hash`,
- implements `Display` as `generator #TARGET.GENERATOR`.

Example display strings:

```text
generator #1.1
generator #1.2
generator #2.1
```

Future:

- this shape naturally supports per-target scheduler trees,
- the runtime can route generator operations to the target scheduler using
  `generator_id.target_id()`.

## Target API

### `TargetConfig`

```rust
pub struct TargetConfig {}
```

MVP behavior:

- empty configuration,
- no target limits,
- no concurrency settings,
- no debug name,
- no scheduling weights.

Future:

- this is the extension point for target limits and scheduling configuration,
- do not add fields until a phase uses them in behavior or tests.

### `Runtime::add_target`

```rust
pub fn add_target(&mut self, config: TargetConfig) -> TargetId;
```

MVP behavior:

- allocates a new `TargetId`,
- creates target state in the runtime,
- stores the empty config,
- initializes that target's generator sequence,
- returns the new id.

The function cannot fail in Phase 0.

Future:

- may validate non-empty target configuration,
- may initialize a per-target scheduler,
- may return an error if target configuration becomes invalid.

### `Runtime::remove_target`

```rust
pub fn remove_target(&mut self, target_id: TargetId) -> bool;
```

MVP behavior:

- returns `false` if the target does not exist,
- removes the target if it exists,
- removes every generator attached to that target,
- removes those generators from the flat scheduler,
- does not remove outputs already present in the output queue,
- returns `true` if a target was removed.

Future:

- may need cancellation/drain policy for in-flight work once background or
  concurrent execution exists,
- may emit runtime events when runtime events are implemented.


## Generator API

### `Generator` trait

```rust
pub trait Generator<E, Err>: Send {
    fn update_ready(&mut self, now: std::time::Instant) -> Readiness;
    fn take_next(&mut self) -> Option<ScheduledWork<E, Err>>;
    fn on_complete(&mut self, completion: &WorkCompletion);
}
```

The generator is the leaf of the scheduling tree. It is supplied by an
application or adapter. It owns any application-specific state needed to decide
when work is ready and how to construct work.

#### `Generator::update_ready`

```rust
fn update_ready(&mut self, now: std::time::Instant) -> Readiness;
```

MVP behavior:

- called by the runtime while scanning candidates,
- called only for generators that still exist,
- reports whether the generator is ready now,
- may report a future `next_ready_at`,
- must not create executable work,
- must not enqueue stale periodic jobs inside the runtime.

Future:

- schedulers may use `next_ready_at` to drive timer wakeups,
- readiness may include cost in a later phase when cost is consumed by a
  scheduler.

#### `Generator::take_next`

```rust
fn take_next(&mut self) -> Option<ScheduledWork<E, Err>>;
```

MVP behavior:

- called only after the runtime selects the generator,
- creates executable work lazily,
- returns `Some(ScheduledWork)` when work is available,
- returns `None` if the generator cannot produce work after all,
- must not be called for not-ready generators.

If `update_ready` said ready but `take_next` returns `None`, `run_once` should
continue scanning other generators during the same runtime step until either one
work item is found or all current generators have been considered.

Future:

- may attach work metadata when scheduling/observability needs it,
- may support cancellation hooks in a later executor phase.

#### `Generator::on_complete`

```rust
fn on_complete(&mut self, completion: &WorkCompletion);
```

MVP behavior:

- called exactly once after work produced by this generator finishes,
- called after the runtime enqueues the corresponding `RuntimeOutput::Work`,
- receives runtime-owned source ids and success/failure outcome,
- does not receive event payloads or error payloads.

Future:

- completion may include timing, retry hints, cancellation, or executor stats,
- periodic generators can use completion to update their next run time.

### `Runtime::add_generator`

```rust
pub fn add_generator<G>(
    &mut self,
    target_id: TargetId,
    generator: G,
) -> Result<GeneratorId, AddGeneratorError>
where
    G: Generator<E, Err> + 'static;
```

MVP behavior:

- verifies that `target_id` exists,
- returns `AddGeneratorError::TargetNotFound { target_id }` if the target does
  not exist,
- allocates a new `GeneratorId` under that target,
- stores the boxed generator,
- records the generator under the target,
- inserts the generator id into the flat round-robin scheduler,
- returns the new generator id.

Phase 0 error shape:

```rust
pub enum AddGeneratorError {
    TargetNotFound { target_id: TargetId },
}
```

Do not add other variants unless Phase 0 implementation or tests need them.

Future:

- later phases will attach the generator to a per-target scheduler,
- configuration or class information may be added only when used by tests and
  behavior.


### `Runtime::remove_generator`

```rust
pub fn remove_generator(&mut self, generator_id: GeneratorId) -> bool;
```

MVP behavior:

- returns `false` if the generator does not exist,
- removes the generator from the runtime,
- removes it from the parent target's generator list,
- removes it from the flat scheduler,
- ensures it is never queried for readiness again,
- does not remove outputs already present in the output queue,
- returns `true` if a generator was removed.

Future:

- may need cancellation/drain policy for in-flight work once concurrent execution
  exists.


## Scheduled work and fallible execution

### `ScheduledWork`

```rust
pub struct ScheduledWork<E, Err> { /* private */ }

impl<E, Err> ScheduledWork<E, Err> {
    pub fn new<F>(future: F) -> Self
    where
        F: std::future::Future<Output = Result<Vec<E>, Err>> + Send + 'static;
}
```

MVP behavior:

- wraps a boxed, pinned, sendable future,
- future output is payload-only: `Result<Vec<E>, Err>`,
- `Ok(Vec<E>)` means the work succeeded and produced zero or more events,
- `Err(Err)` means the work failed,
- scheduled work does not construct `WorkSuccess`,
- scheduled work does not construct `WorkError`,
- scheduled work does not provide target or generator ids.

Runtime-owned enrichment:

- runtime awaits `ScheduledWork`,
- runtime attaches `target_id` and `generator_id`,
- runtime constructs `WorkSuccess<E>` or `WorkError<Err>`,
- runtime enqueues `RuntimeOutput::Work(result)`.

Future:

- scheduled work may receive metadata or cancellation support,
- success/failure wrappers can gain runtime-provided fields without changing
  generator work futures.

### `WorkSuccess`

```rust
pub struct WorkSuccess<E> {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub events: Vec<E>,
}
```

MVP behavior:

- constructed only by the runtime,
- identifies the target and generator that produced the work,
- contains events in the exact order returned by the work future.

Future:

- may add runtime-provided timing, attempt number, or statistics when those are
  implemented.

### `WorkError`

```rust
pub struct WorkError<Err> {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub error: Err,
}
```

MVP behavior:

- constructed only by the runtime,
- identifies the target and generator whose work failed,
- carries the generic work error value returned by the work future.

Future:

- may add runtime-provided timing, retry classification, or executor facts.

### `WorkCompletion`

```rust
pub struct WorkCompletion {
    pub target_id: TargetId,
    pub generator_id: GeneratorId,
    pub outcome: WorkOutcome,
}

pub enum WorkOutcome {
    Succeeded,
    Failed,
}
```

MVP behavior:

- constructed by the runtime,
- sent to the generator that created the completed work,
- contains ids and outcome only,
- does not expose events or errors to `on_complete`.

Future:

- may include runtime-provided latency, cancellation status, or scheduler stats.

## Runtime output

```rust
pub type WorkResult<E, Err> = Result<WorkSuccess<E>, WorkError<Err>>;

pub enum RuntimeOutput<E, Err, R = std::convert::Infallible> {
    Work(WorkResult<E, Err>),
    Runtime(R),
}
```

MVP behavior:

- all completed work appears as `RuntimeOutput::Work`,
- successful work is `RuntimeOutput::Work(Ok(WorkSuccess { ... }))`,
- failed work is `RuntimeOutput::Work(Err(WorkError { ... }))`,
- runtime events are not emitted in Phase 0,
- default runtime event type is `Infallible`,
- with the default runtime event type, `RuntimeOutput::Runtime(_)` is not
  constructible in normal use,
- if a caller explicitly supplies an inhabited `R`, the variant exists in the
  public type, but Phase 0 runtime code still must not emit it.

Future:

- runtime events can be feature-gated and emitted as `RuntimeOutput::Runtime(R)`,
- an async stream API can be added above the same output item type,
- the output stream should remain ordered across work and runtime events.


## Runtime execution API

### `Runtime::new`

```rust
pub fn new() -> Self;
```

MVP behavior:

- creates an empty runtime,
- initializes id counters,
- initializes empty target and generator maps,
- initializes the flat round-robin scheduler,
- initializes an empty output queue.

No configuration is accepted in Phase 0.

Future:

- runtime configuration may be introduced when it affects real behavior.

### `Runtime::run_once`

```rust
pub async fn run_once(&mut self) -> RunOnce;
```

MVP behavior:

`run_once` performs one complete runtime step:

1. asks the flat scheduler for generator candidates in round-robin order,
2. skips candidate ids that no longer exist,
3. calls `update_ready(now)` on candidates,
4. skips generators that are not ready,
5. calls `take_next` on the first ready candidate,
6. if `take_next` returns `None`, continues scanning,
7. awaits one returned `ScheduledWork`,
8. converts `Ok(Vec<E>)` into `WorkSuccess<E>`,
9. converts `Err(Err)` into `WorkError<Err>`,
10. enqueues `RuntimeOutput::Work(result)`,
11. calls `on_complete` on the originating generator,
12. returns `RunOnce::Executed`.

If no generator produces work after a full scan, returns `RunOnce::Idle`.

Ordering requirement:

- enqueue output before calling `on_complete`,
- call `on_complete` exactly once for executed work,
- never call `on_complete` when no work was executed.

Future:

- a later runtime driver may repeatedly call `run_once`,
- a background/concurrent executor may replace the sequential execution path,
- richer run results may be added only when needed.

### `RunOnce`

```rust
pub enum RunOnce {
    Executed,
    Idle,
}
```

MVP behavior:

- `Executed`: one work item was selected, awaited, output, and completed,
- `Idle`: no work item was available.

Work failure still returns `RunOnce::Executed` because the runtime executed a
fallible work item and emitted a failure output.

### `Runtime::next_output`

```rust
pub fn next_output(&mut self) -> Option<RuntimeOutput<E, Err, R>>;
```

MVP behavior:

- pops the oldest queued output,
- returns `None` when the queue is empty,
- preserves FIFO output order.

Future:

- an async stream API may be layered over the same queue semantics.

### `Runtime::drain_outputs`

```rust
pub fn drain_outputs(&mut self) -> Vec<RuntimeOutput<E, Err, R>>;
```

MVP behavior:

- removes all currently queued outputs,
- returns them in FIFO order,
- does not run the scheduler or executor.

Future:

- batch drain may include size limits if bounded queues are added.

## Internal architecture

Phase 0 runtime should use an internal shape equivalent to:

```text
Runtime<E, Err, R = Infallible>
  inner: RuntimeInner<E, Err, R>

RuntimeInner
  next_target_id: u64
  targets: HashMap<TargetId, TargetState>
  generators: HashMap<GeneratorId, GeneratorSlot<E, Err>>
  scheduler: FlatRoundRobin
  outputs: VecDeque<RuntimeOutput<E, Err, R>>

TargetState
  next_generator_id: u64
  generators: Vec<GeneratorId>

GeneratorSlot<E, Err>
  generator: Box<dyn Generator<E, Err>>

FlatRoundRobin
  order/cursor data only
```

`TargetConfig` is empty in Phase 0. `add_target(config)` accepts it as a future
extension point, but the implementation may store it or ignore it because there
are no user-observable fields. Do not add target config read APIs or unused
placeholder fields.

Because `GeneratorId` contains `TargetId`, `GeneratorSlot` does not need to store
a separate target id unless implementation convenience requires it.

## Flat round-robin scheduler

The Phase 0 scheduler is a pure synchronous data structure.

It must:

- store generator ids in insertion order,
- maintain round-robin cursor state,
- support insertion of a generator id,
- support removal of a generator id,
- provide candidates for a runtime scan,
- be deterministic.

It must not:

- call generators,
- inspect readiness,
- create work,
- await futures,
- enqueue output,
- know Redfish or application semantics,
- know target limits or costs.

Required scan behavior:

1. `run_once` scans at most the generators that are present when the scan starts.
2. The scheduler returns candidates in flat global round-robin order.
3. The scheduler advances the cursor as candidates are considered, including
   candidates that are not ready or whose `take_next` returns `None`.
4. If a candidate produces work, the next `run_once` starts after that candidate.
5. If no candidate produces work, the next `run_once` starts after the full scan.
6. Removed generator ids are deleted from scheduler state and are never returned
   again.

Future:

- this module can be replaced by hierarchical root/target scheduling,
- tests should avoid depending on private cursor details.


## BMC-explorer-like Phase 0 test

Phase 0 must include a fake discovery-flow test that mimics modern Site Explorer
at the runtime boundary without depending on Carbide or Redfish.

The test should model this flow:

1. Application creates `Runtime<FakeExplorerEvent, FakeExplorerError>`.
2. Application adds one or more targets representing BMC endpoints.
3. Application adds an initial service-root generator under each target.
4. `run_once` executes a service-root generator.
5. Runtime emits `RuntimeOutput::Work(Ok(WorkSuccess { ... }))` with a fake
   service-root discovery event.
6. Application drains outputs.
7. Application-owned policy inspects the fake service-root event.
8. Application adds fake system and chassis generators under the same target.
9. More `run_once` calls emit fake system/chassis discovery events.
10. Application drains outputs and builds a fake exploration report externally.

This test must prove:

- runtime owns target/generator control,
- runtime does not own discovery policy,
- generators can be added in response to outputs,
- generator ids include target ids,
- output successes include target and generator ids,
- event ordering is deterministic,
- application builds the final report outside the runtime.

The fake event type can be shaped like:

```rust
enum FakeExplorerEvent {
    ServiceRootDiscovered {
        systems: Vec<String>,
        chassis: Vec<String>,
    },
    SystemDiscovered {
        system_id: String,
    },
    ChassisDiscovered {
        chassis_id: String,
    },
}
```

The fake error type can be simple, for example:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeExplorerError(String);
```

Do not include target ids inside fake events just to satisfy runtime needs; the
runtime-provided `WorkSuccess` and `WorkError` already carry source identity.

## Required tests

All tests must use fake generators, fake events, and fake errors only.

### Test: target ids are generated

- Add two targets.
- Verify ids are distinct.
- Verify display strings are `target #1` and `target #2`.

### Test: generator ids include target ids

- Add two targets.
- Add generators under each target.
- Verify `generator_id.target_id()` returns the parent target.
- Verify display strings such as `generator #1.1`, `generator #1.2`, and
  `generator #2.1`.

### Test: add generator requires existing target

- Create a `TargetId` that is not in the runtime or use an id from a removed
  target.
- Attempt to add a generator.
- Verify an `AddGeneratorError` is returned.

### Test: flat round-robin order

- Add one target.
- Add three always-ready generators A, B, and C.
- Each generator returns one event per selection.
- Repeated `run_once` calls should produce events in order:

```text
A, B, C, A, B, C
```

### Test: output event ordering inside one work item

- Add one generator whose work returns multiple events.
- Run once.
- Drain outputs.
- Verify events inside `WorkSuccess.events` preserve returned order.

### Test: work error output

- Add one generator whose work returns `Err(FakeExplorerError)`.
- Run once.
- Verify `RunOnce::Executed`.
- Drain outputs.
- Verify output is `RuntimeOutput::Work(Err(WorkError { ... }))`.
- Verify `WorkError` contains runtime-provided target and generator ids.


### Test: completion callback on success

- Add one successful generator.
- Run once.
- Verify the generator observed exactly one completion with
  `WorkOutcome::Succeeded`.

### Test: completion callback on failure

- Add one failing generator.
- Run once.
- Verify the generator observed exactly one completion with
  `WorkOutcome::Failed`.

### Test: not-ready generator is skipped

- Add one not-ready generator and one ready generator.
- Run once.
- Verify `take_next` was not called on the not-ready generator.
- Verify ready generator ran.

### Test: work is created only when selected

- Add multiple generators.
- Count `take_next` calls.
- Verify a generator's `take_next` count increases only when selected, not when
  added and not when another generator runs.

### Test: remove generator

- Add generators A and B.
- Remove B.
- Run multiple times.
- Verify B is never queried for readiness and never produces output.

### Test: remove target removes generators

- Add target T with generators A and B.
- Remove T.
- Run once.
- Verify runtime is idle if no other generators exist.
- Verify A and B are not queried again.

### Test: produced outputs survive removal

- Run a generator and produce output.
- Remove the generator or target before draining output.
- Verify previously produced output remains drainable.

### Test: BMC-explorer-like discovery flow

Implement the fake discovery-flow described above. The final fake exploration
report must be built by test/application code, not by the runtime.

## Implementation guardrails

- Keep the runtime generic over `E`, `Err`, and `R`.
- Keep `R = Infallible` as the default runtime event type.
- Do not emit runtime events in Phase 0.
- Keep target config empty.
- Do not add pause/resume.
- Do not add `run_until_idle`.
- Do not add cost/class/budget fields.
- Do not add target limits.
- Do not spawn background tasks.
- Do not make the scheduler async.
- Do not let scheduled work construct `WorkSuccess` or `WorkError`.
- Runtime must attach target and generator ids to outputs.
- Runtime must preserve output order.
- Removed generators must not be queried again.
- Removed targets must remove their generators.
- Tests must be deterministic and avoid timing-based assertions.

## Completion criteria

Phase 0 is complete when:

- `scraper` crate exists and is named `nv-redfish-scraper`,
- it is part of the workspace,
- it builds without Redfish or Carbide dependencies,
- target and generator ids behave as specified,
- generator ids expose parent target ids,
- flat round-robin scheduling works,
- `run_once` executes at most one work item,
- work success and work failure both appear as ordered outputs,
- runtime constructs `WorkSuccess` and `WorkError`,
- completion callbacks are called once per executed work item,
- removing generators and targets works,
- the fake BMC-explorer-like discovery-flow test passes,
- no unused placeholder scheduler/cost/limit APIs are present.

## Next phase preview

Phase 1 should introduce the first real hierarchy while preserving the Phase 0
public concepts:

- target-aware scheduling internals,
- root scheduler over target schedulers,
- per-target generator scheduling,
- target limits if tests consume them,
- global limits if tests consume them,
- runtime statistics only when used by behavior/tests.

Redfish adapter work should wait until the generic runtime boundary is stable
enough for adapter generators to consume.
