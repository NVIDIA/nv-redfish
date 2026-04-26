# Root scheduler

The root scheduler is the top-level scheduling item in the generic runtime. It is
independent of Redfish and schedules target schedulers using only abstract
scheduling metadata.

For Redfish use cases, a target is usually a BMC. The root scheduler does not
know that.

## Responsibilities

- Protect runtime-wide capacity.
- Enforce global maximum in-flight work.
- Enforce global capacity or budget when configured.
- Schedule fairly among ready target schedulers.
- Prevent one target from consuming all runtime resources.
- Expose global scheduling statistics.

## Children

The root scheduler's children are target schedulers. A child is identified by an
opaque target id.

A target scheduler is ready when:

- it has at least one local generator ready to produce work,
- local target limits allow another dispatch,
- and the next work item can be represented by scheduling metadata such as cost,
  class, and readiness.

## Pull-based flow

```text
root scheduler is triggered
  -> update_ready(now) on not-ready target schedulers
  -> rebuild/update root ready set
  -> select one ready target according to root discipline
  -> call take_next() on selected target scheduler
  -> receive ScheduledWork<E> for executor dispatch
```

The executable work is created below the root scheduler. The root scheduler only
applies global scheduling constraints.

## Notes

The initial implementation can use a simple fair discipline if needed, but the
interface should allow WRR/DRR-like root scheduling later.
