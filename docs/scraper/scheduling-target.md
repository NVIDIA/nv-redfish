# Target scheduler

A target scheduler protects one target. For Redfish use cases the target is
usually one BMC, but the generic runtime treats the target id opaquely.

The target scheduler is a child scheduling item of the root scheduler and the
parent of generator leaves.

## Responsibilities

- Protect one target from overload.
- Enforce per-target maximum in-flight work.
- Enforce per-target capacity or budget.
- Schedule fairly among local generators/classes.
- Account for weighted work costs.
- Prevent high-frequency cheap flows from permanently starving expensive flows.
- Prevent expensive background flows from starving high-priority flows.
- Expose per-target and per-class/generator statistics.

## Children

The target scheduler's children are generators.

For Redfish use cases, examples include:

- service-root/bootstrap generator,
- systems collection generator,
- chassis collection generator,
- firmware generator,
- sensor generators,
- OEM-specific generators.

The scheduler does not understand those meanings; they are generator identities
and classes from the scheduler's point of view.

## Pull-based flow

```text
target scheduler update_ready(now)
  -> update budget/time state
  -> update_ready(now) on not-ready child generators
  -> rebuild/update local ready set
  -> report readiness to root scheduler

target scheduler take_next()
  -> select one ready generator according to local discipline
  -> call take_next() on selected generator
  -> reserve local budget/in-flight state
  -> return ScheduledWork<E> to root scheduler / executor path
```

## Dynamic hierarchy

The child set is dynamic. The application can add or remove generators when its
model or policy changes.

For Redfish, an application may start with a service-root generator, then add
chassis, system, manager, firmware, sensor, BIOS, interface, or OEM generators in
response to events.

Whenever children are added, removed, or materially changed, local readiness must
be invalidated or recomputed. The scheduler must not keep stale ready-set entries
for removed children.

## Notes

DRR-like scheduling is expected to be useful locally because work costs can vary
substantially between cheap sensor reads and expensive inventory traversal.
