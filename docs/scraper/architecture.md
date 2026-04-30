# Scraper architecture

The scraper design has three blocks:

1. user application,
2. generic runtime,
3. Redfish adapter.

The user application owns policy and models. The runtime owns scheduling,
execution, and event delivery. The Redfish adapter binds the generic runtime to
`nv-redfish` objects and generated Redfish payloads.

## Block overview

```text
User application
  creates initial generators
  updates runtime tree
  consumes ordered outputs
  updates application model
        |
        v
Generic runtime
  scheduler hierarchy
  executor
  output queue
        |
        v
Redfish adapter
  feature-gated generator builders
  nv-redfish object operations
  Redfish work events with generated EntityPayload
```

Control flows from the application into the runtime. Ordered outputs flow from
the runtime back to the application.

## User application

The user application decides what to discover and what the data means.

Examples are Carbide Site Explorer, Carbide Health, or another project using
`nv-redfish` scraping.

Responsibilities:

- create BMC clients and initial `nv-redfish` objects, such as `ServiceRoot<B>`,
- create initial generators or ask the Redfish adapter to build them,
- consume the runtime output stream,
- update its own model/projections,
- add, remove, pause, resume, or reconfigure generators in response to outputs,
- persist outputs or state if desired,
- expose APIs, metrics, alerts, or reports.

The application owns discovery policy. For example, after receiving a service
root work event it may decide to add chassis generators, system generators,
firmware generators, or none of them.

Carbide Site Explorer still owns `EndpointExplorationReport`, `to_model()`,
compare mode, and Carbide inventory semantics. Carbide Health still owns metric
names, labels, export mapping, overrides, and health sinks.

## Generic runtime

The runtime is not Redfish-specific. It is parameterized by a work event type
`E`. Its ordered output stream may also contain feature-gated runtime events.

Responsibilities:

- maintain the scheduler hierarchy,
- keep generators as leaves of that hierarchy,
- run selected work through the executor,
- publish produced work events and optional runtime events to an ordered output
  queue or stream,
- expose scheduler/generator statistics,
- expose a control API for updating the scheduler tree.

The runtime does not know `Bmc`, `ODataId`, `Chassis<B>`, `Sensor<B>`, or any
generated Redfish schema type.

The runtime is described in detail in [Runtime](runtime.md).

## Redfish adapter

The Redfish adapter is the Redfish-specific binding for the generic runtime.

Responsibilities:

- provide feature-gated generator builders,
- create generators that close over valid `nv-redfish` objects,
- call valid methods on objects such as `ServiceRoot<B>`, `Chassis<B>`,
  `ComputerSystem<B>`, sensor links, firmware inventory objects, and other
  compiled wrappers,
- emit Redfish work events using generated `EntityPayload`,
- preserve `ODataId`, optional parent `ODataId`, ETag, scrape metadata, errors,
  and expanded payload data.

The adapter does not own application discovery policy. It provides pieces the
application can add to the runtime tree.

The Redfish adapter is described in detail in [Redfish adapter](redfish-adapter.md).

## Example: BMC Explorer flow

```text
BMC Explorer creates BMC client
BMC Explorer creates ServiceRoot<B>
BMC Explorer asks Redfish adapter for a service-root generator
BMC Explorer adds generator to runtime
Runtime schedules and executes generator
Runtime emits Redfish service-root work output
BMC Explorer consumes output and updates its model
BMC Explorer decides what else is needed
BMC Explorer adds chassis/system/manager/firmware generators
Runtime emits more outputs
BMC Explorer builds EndpointExplorationReport externally
```

The runtime stays generic. The Redfish adapter knows how to fetch Redfish data.
The application decides the exploration plan.
