# Scraper runtime

The runtime is the generic black box between the application and executable
work. It is not Redfish-specific. It is parameterized by an application work
event type `E` and a work error type `Err`.

The runtime owns scheduling, execution, event delivery, and runtime statistics.
The application owns policy and models. Adapters provide generators and work
event types.

Detailed scheduling internals are described in [Scheduling](scheduling.md).

## Block overview

```text
Application / adapter
  adds targets
  adds generators
  updates generator state/config
  consumes ordered outputs
        |
        v
Runtime<E, Err>
  control API
  scheduler tree
  executor
  output queue
  stats
        |
        v
RuntimeOutput<E, Err>
```

The runtime does not know `Bmc`, `ODataId`, `nv-redfish` wrappers, Redfish schema
types, or application domain models.

## Inputs

The runtime accepts control operations from the application.

Target operations:

```rust
add_target(target_id, limits)
remove_target(target_id)
update_target_limits(target_id, limits)
pause_target(target_id)
resume_target(target_id)
```

Generator operations:

```rust
add_generator(target_id, generator_id, generator)
remove_generator(generator_id)
update_generator(generator_id, config)
pause_generator(generator_id)
resume_generator(generator_id)
trigger_generator(generator_id)
```

A target is opaque to the runtime. In Redfish use cases it is usually a BMC, but
that meaning belongs to the application or Redfish adapter.

## Outputs

The runtime exposes one ordered output stream.

Conceptual shape:

```rust
pub type WorkResult<E, Err> = Result<WorkSuccess<E>, WorkError<Err>>;

pub struct WorkSuccess<E> {
    pub events: Vec<E>,
    pub stats: WorkStats,
}

pub struct WorkError<Err> {
    pub error: Err,
    pub stats: WorkStats,
}

pub enum RuntimeOutput<E, Err, R = RuntimeEventType> {
    Work(WorkResult<E, Err>),
    Runtime(R),
}
```

`RuntimeOutput::Work(Ok(_))` contains events produced by successful scheduled
work. For Redfish scraping, `E` is a Redfish event type from the Redfish adapter.
For tests, `E` can be a fake event type.

`RuntimeOutput::Work(Err(_))` contains a generic work error `Err` produced by
failed scheduled work. The runtime owns the wrapper and can attach runtime-owned
metadata or statistics around the application or adapter error.

`RuntimeOutput::Runtime(R)` contains out-of-band runtime events, when compiled
in. Runtime events describe scheduler, executor, or queue facts such as lag,
throttling, starvation, and queue pressure.

The combined stream preserves causal ordering between successful work, failed
work, and runtime events.

## Runtime event feature

Out-of-band runtime events are compile-time feature gated.

When enabled:

```rust
pub type RuntimeEventType = RuntimeEvent;
```

When disabled:

```rust
pub type RuntimeEventType = core::convert::Infallible;
```

With runtime events disabled, the runtime event payload type is uninhabited and
`RuntimeOutput::Runtime(_)` cannot be constructed. Runtime event emission code
should not be compiled.

Runtime event examples when enabled:

- generator lagging,
- generator recovered,
- generator starved,
- target throttled,
- global throttled,
- event queue pressure,
- work started,
- work completed,
- work failed,
- scheduler statistics snapshot.

## Generator interface

A generator is supplied by the application or adapter. The runtime treats it as a
scheduling leaf that can produce executable work.

Conceptual shape:

```rust
pub trait Generator<E, Err> {
    fn update_ready(&mut self, now: Instant) -> Readiness;
    fn take_next(&mut self) -> Option<ScheduledWork<E, Err>>;
    fn on_complete(&mut self, completion: &WorkCompletion);
}
```

The runtime does not inspect the generator's internal state. It only asks for
readiness, pulls work when selected, and reports completion.

## Scheduled work interface

Scheduled work is the executable unit returned by a selected generator.

```rust
pub struct ScheduledWork<E, Err> {
    pub meta: WorkMeta,
    pub future: Pin<Box<dyn Future<Output = ScheduledWorkResult<E, Err>> + Send + 'static>>,
}

pub type ScheduledWorkResult<E, Err> = Result<Vec<E>, Err>;
```

The future may close over anything needed by the generator. For Redfish adapter
use, it may close over `ServiceRoot<B>`, `Chassis<B>`, sensor links, or other
`nv-redfish` objects. The runtime runs the future, preserves any returned events
in order, wraps success as `WorkSuccess<E>`, wraps failure as `WorkError<Err>`,
and publishes the result as `RuntimeOutput::Work`.

Work statistics remain runtime-owned. The runtime may attach `WorkStats` or other
runtime metadata to `WorkSuccess` and `WorkError`; scheduled work should not need
to fabricate runtime statistics itself.

## Runtime responsibilities

The runtime is responsible for:

- maintaining the target/generator tree,
- selecting ready generators according to scheduler policy,
- executing selected work,
- reporting completion back to generators and schedulers,
- publishing work result events,
- optionally publishing runtime events when the feature is enabled,
- exposing runtime statistics,
- preventing removed or paused generators from being scheduled.

The runtime is not responsible for:

- deciding which Redfish resources should be discovered,
- creating `nv-redfish` objects,
- interpreting Redfish payloads,
- building application read models,
- persisting events durably,
- performing application-specific retries or policy decisions unless configured
  through generic runtime controls.

## Output queue interface

The output queue accepts work results from completed work and optional runtime
events generated by the runtime. It exposes ordered `RuntimeOutput<E, Err>` items
to the application.

The output queue should expose:

- stream subscription or polling API,
- one-shot batch drain API,
- queue length/pressure statistics,
- dropped or rejected output counts if bounded queues are configured.

Backpressure policy is runtime configuration. Periodic generator overload should
still be represented primarily by generator lag and missed intervals, not by
building stale work queues.

## Statistics interface

The runtime should expose statistics for:

- global scheduler state,
- per-target scheduler state,
- per-generator state,
- in-flight work,
- work latency,
- generator lag,
- missed intervals,
- event queue pressure,
- executor errors.

Statistics may be exposed as snapshots. When runtime events are enabled,
statistics may also be emitted as ordered runtime events.

## Testing

The runtime should be testable without Redfish.

Tests can use fake generators, fake scheduled work, fake work events, and either
fake or disabled runtime events. This allows scheduler, executor, output queue,
and control API behavior to be tested without BMC mocks or HTTP.
