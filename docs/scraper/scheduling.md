# Scraper scheduling overview

Scheduling is part of the generic runtime. It is independent of Redfish,
`nv-redfish`, BMC transports, generated schema types, and application event
payloads.

For the full runtime context, see [Runtime](runtime.md).

## Scope

The scheduler operates on abstract scheduling metadata:

- target identity,
- generator identity,
- class identity,
- readiness,
- next ready time,
- next request cost,
- in-flight state,
- capacity/budget state.

The scheduler does not know whether selected work will fetch Redfish service
root, chassis, sensors, firmware, or a non-Redfish resource.

## Core idea

Periodic flows are modeled as generators, not queues of pre-created jobs.

A generator reports readiness and estimated cost. It creates executable work only
when the scheduler selects it.

```text
root scheduler is triggered
        |
        v
root scheduler updates readiness of target schedulers
        |
        v
root scheduler selects one ready target scheduler
        |
        v
target scheduler selects one ready generator
        |
        v
selected generator creates ScheduledWork<E>
        |
        v
executor runs ScheduledWork<E>
        |
        v
events are published and completion is reported
```

If a periodic generator cannot run at the requested rate, it accumulates lag. It
does not accumulate stale queued jobs.

## Scheduling item abstraction

Schedulers and generators can both be viewed as scheduling items.

Conceptual shape:

```rust
pub struct CostUnits(pub u64);

pub struct Readiness {
    pub ready: bool,
    pub next_update_at: Option<Instant>,
    pub next_cost: Option<CostUnits>,
}

pub trait SchedulingItem {
    type Request: ScheduledRequest;

    fn update_ready(&mut self, now: Instant) -> Readiness;
    fn take_next(&mut self) -> Option<Self::Request>;
}

pub trait ScheduledRequest {
    fn cost(&self) -> CostUnits;
}
```

`CostUnits` is a concrete newtype over `u64`, not a generic parameter.

## Hierarchy

```text
Root scheduler
  protects runtime-wide capacity
  schedules among target schedulers

Target scheduler
  protects one target's capacity
  schedules among local generators/classes

Generator
  leaf of the scheduling hierarchy
  creates work only when selected
```

For Redfish, a target is typically a BMC. The scheduler still treats it as an
opaque target.

Detailed scheduler notes:

- [Root scheduler](scheduling-root.md)
- [Target scheduler](scheduling-target.md)

## Dynamic tree

The scheduling tree is dynamic. The application can add, remove, pause, resume,
or reconfigure generators through the runtime control API.

When tree shape changes, affected readiness must be invalidated or recomputed.
Schedulers must not keep stale ready-set entries for removed generators.

## QoS model

The scheduling model is inspired by network QoS scheduling.

```text
generator/class        ~= traffic class / flow
scheduled work         ~= packet
work cost              ~= packet size
target capacity        ~= link bandwidth
target interval        ~= service objective
lag/missed interval    ~= scheduling delay / SLA miss
```

WRR/DRR-like scheduling is expected to be useful because work items can have very
different costs. Deficit-style accounting is preferred over plain round-robin.

Desired properties:

- runtime-wide load is shaped,
- per-target load is shaped,
- classes receive configured service shares,
- expensive work eventually runs,
- high-priority flows receive preferred service,
- low-rate background flows are not permanently starved.

## Overload signals

The runtime should report overload through signals such as:

- generator lag,
- missed target intervals,
- budget starvation,
- in-flight saturation,
- actual interval versus requested interval,
- event queue pressure.

Queue depth of periodic jobs is not a primary signal because periodic jobs are
not pre-created.

## Testing

Scheduler tests should not require Redfish. They can use fake generators, fake
costs, fake readiness times, fake completions, and fake events.
