# nv-redfish scraper requirements

## 1. Scope

- The scraper MUST be a reusable library in the `nv-redfish` workspace.
- The scraper MUST be publishable to crates.io.
- The scraper MUST be usable outside Carbide.
- The scraper MUST support customizable Redfish data retrieval.
- The scraper MUST support one-shot and periodic scraping.
- The scraper MUST initially be read-oriented.
- The scraper MUST NOT contain Carbide-specific model conversion, DB/API/Vault
  logic, or dependencies on Carbide crates.

## 2. Architecture split

- The scraper architecture MUST separate:
  - user application,
  - generic runtime,
  - Redfish adapter.
- The user application MUST own discovery policy, domain model, projections,
  persistence, APIs, metrics, and mutations.
- The generic runtime MUST own scheduler hierarchy, executor, output queue, and
  control API.
- The Redfish adapter MUST bind the runtime to `nv-redfish` objects and generated
  Redfish payloads.

## 3. Generic runtime

- The runtime MUST be independent of Redfish.
- The runtime MUST be parameterized by an application work event type `E`.
- The runtime MUST NOT know about `Bmc`, `ODataId`, `Chassis<B>`, `Sensor<B>`, or
  generated Redfish schema types.
- The runtime MUST provide a scheduler tree update/control API.
- The runtime MUST provide an ordered output queue or stream.
- Runtime output MUST preserve causal ordering between work events and runtime
  events when runtime events are compiled in.
- The runtime MUST support one-shot batch collection and periodic streaming.
- The runtime MUST expose scheduler, generator, executor, and queue statistics.

## 4. Runtime output and OOB events

- Runtime output MUST have a shape equivalent to `RuntimeOutput<E, R>` with work
  events and optional runtime events.
- Work events MUST be application or adapter events of type `E`.
- Runtime events MUST describe out-of-band scheduler, executor, and queue facts
  such as lag, starvation, throttling, work completion, work failure, and queue
  pressure.
- Runtime events MUST be compile-time feature gated.
- When runtime events are disabled, the runtime event payload type MUST be
  uninhabited, for example `core::convert::Infallible`.
- When runtime events are disabled, runtime event emission code MUST NOT be
  compiled and `RuntimeOutput::Runtime(_)` MUST NOT be constructible.

## 5. Scheduler tree

- The runtime MUST use hierarchical scheduling:
  - root/global scheduler,
  - per-target scheduler,
  - generators as leaves.
- For Redfish, a target is usually a BMC, but the runtime MUST treat target ids
  opaquely.
- The root scheduler MUST protect runtime-wide capacity and global in-flight
  work.
- The target scheduler MUST protect per-target capacity and per-target in-flight
  work.
- The target scheduler MUST schedule among local generators/classes.
- Schedulers MUST operate only on abstract metadata: identity, class, readiness,
  next ready time, cost, in-flight state, and budget/capacity state.
- Schedulers MUST NOT be parameterized by concrete Redfish request type.
- The scheduler tree MUST support adding, removing, pausing, resuming,
  triggering, and reconfiguring generators.
- Tree changes MUST invalidate or recompute affected readiness.

## 6. Generators

- Periodic flows MUST be modeled as generators, not queues of pre-created jobs.
- A generator MUST be stateful.
- A generator MUST expose readiness and next ready time.
- A generator MUST expose estimated next work cost before dispatch.
- A generator MUST create executable work only after the scheduler selects it.
- A generator MUST update state after completion.
- A generator MUST report lag or missed target intervals when it cannot run at
  the requested rate.
- The runtime MUST NOT accumulate stale periodic jobs.

## 7. Executor and output queue

- The executor MUST run scheduled work selected by the scheduler.
- The executor MUST not decide what work should exist.
- The executor MUST report completion to schedulers and generators.
- The executor MUST publish work result events to the output queue as ordered
  work outputs.
- The output queue MUST be generic over work event type `E`.
- The output queue SHOULD expose pressure, drop, or backpressure statistics when
  configured.

## 8. Redfish adapter and transport

- The Redfish adapter MUST be generic over `B: nv_redfish::Bmc` on the fetch side.
- The adapter MUST work with HTTP BMCs, mock BMCs, and custom BMC
  implementations.
- The adapter MUST provide feature-gated generator builders for Redfish
  capabilities.
- Adapter generators SHOULD close over valid `nv-redfish` objects such as
  `ServiceRoot<B>`, `Chassis<B>`, `ComputerSystem<B>`, sensor links, firmware
  inventory objects, or other wrappers.
- The adapter MUST NOT use a detached command language that can pair invalid
  commands with Redfish objects.
- The adapter MUST allow applications to add only the generators needed for their
  target and policy.

## 9. Compile-time feature gating

- The scraper MUST use Cargo features to control which Redfish adapter
  capabilities are compiled.
- If a capability feature is disabled:
  - related generator builders MUST NOT be compiled,
  - related config fields MUST NOT be compiled,
  - related event payload variants MUST NOT be compiled because the schema type
    is not compiled,
  - related fetch code MUST NOT be compiled.
- Disabled capability means it cannot be requested through typed APIs.
- Scraper feature definitions SHOULD be documented in [Features](features.md).

## 10. Generated EntityPayload and serialization

- The `nv-redfish` CSDL compiler SHOULD optionally generate an `EntityPayload`
  enum with one variant for each compiled entity type.
- The Redfish adapter MUST require this generated feature.
- `EntityPayload` MUST preserve generated Redfish schema data and avoid a
  parallel scraper domain model.
- Generated support SHOULD expose entity kind, `@odata.id`, and `@odata.etag`
  when present.
- The `nv-redfish` CSDL compiler SHOULD support a feature or codegen flag that
  derives `serde::Serialize` for generated read/entity data.
- Serialized Redfish work events MUST contain read-side data and metadata, not
  execution handles such as `Chassis<B>` or `ComputerSystem<B>`.
- Distributed transfer, durable event storage, and replay policy are application
  responsibilities, but the generated data and scraper event types SHOULD support
  serialization when the relevant features are enabled.
- Scraper semantics such as inserted, updated, refreshed, stale, removed, and
  failed MUST remain outside the CSDL compiler.

## 11. Redfish events and metadata

- The Redfish adapter MUST emit typed Redfish work events using `EntityPayload`.
- Resource events MUST include source BMC identity.
- Resource events MUST include resource `ODataId`.
- Resource events MUST include optional parent `ODataId` when known.
- Resource events MUST include scrape metadata such as scraped time, latency,
  freshness/generation/change hints, and errors when applicable.
- Resource events MUST distinguish at least:
  - inserted,
  - updated,
  - refreshed without change,
  - fetch failed.
- Resource events SHOULD support stale and removed resources.
- Public Redfish events MUST avoid exposing `B` or `Chassis<B>`/`ComputerSystem<B>`
  execution handles.

## 12. Expand handling

- The adapter MUST preserve expanded Redfish payloads.
- If fetched data contains expanded `NavProperty<T>`, the emitted/stored
  `EntityPayload` MUST retain that expanded data.
- The adapter MAY also emit separate child resource events for expanded children.
- Child events SHOULD include `parent_odata_id` when the parent is known.
- The adapter MUST NOT collapse expanded data to links only as the primary event
  payload.
- Expand policy is adapter/generator behavior, not generic runtime behavior.

## 13. Optional reconstruction

- Reconstruction of Redfish execution state SHOULD be an optional Redfish adapter
  helper, not generic runtime behavior.
- Reconstruction SHOULD use `ODataId` and `parent_odata_id` to rebuild hierarchy.
- Reconstruction MAY rebuild scheduler trees from stored events or records.
- Reconstruction MAY reconstruct `nv-redfish` wrappers from stored schema payloads
  plus a live `Bmc` if `nv-redfish` exposes suitable constructors.
- Event consumers MUST NOT be required to reconstruct `B`-typed execution objects.

## 14. Runtime customization

- Applications MUST be able to specify exactly which compiled capabilities they
  need by choosing which generators to add.
- Applications MUST be able to perform narrow scraping, for example only GPU
  sensors, thermal sensors, firmware, attestation, chassis inventory, or selected
  system subresources.
- The scraper MUST NOT require full BMC inventory scraping when only a subset is
  requested.
- For compiled capabilities, applications MUST be able to distinguish not
  requested, requested missing, requested failed, and requested successful.

## 15. Scheduling and QoS

- The runtime MUST protect each target from overload.
- The runtime MUST protect total process capacity.
- The runtime MUST support per-target and global maximum concurrency.
- The runtime MUST support weighted work costs.
- The runtime SHOULD support WRR/DRR-like scheduling.
- The runtime SHOULD support class weights or equivalent service shares.
- The runtime SHOULD avoid starvation of expensive or low-rate generators.
- The runtime SHOULD expose actual interval versus requested interval.

## 16. Overload and observability

- The runtime MUST NOT rely on periodic job queue depth as the main overload
  signal.
- The runtime MUST report overload using generator lag, missed periods,
  in-flight saturation, budget starvation, actual interval versus target
  interval, and event queue pressure.
- The runtime MUST expose per-target, per-class, per-generator, and global
  scheduling/load information.
- When runtime events are enabled, overload and scheduler facts SHOULD be exposed
  as ordered runtime events.
- The Redfish adapter SHOULD expose error rates and latency by BMC/profile/request
  type where applicable.

## 17. Carbide migration boundaries

- Carbide Site Explorer MUST keep `EndpointExplorationReport`, `to_model()`,
  compare/migration reporting, and Carbide inventory semantics outside the
  generic scraper.
- Carbide Health MUST keep health metric names, labels, Prometheus/OTLP mapping,
  overrides, and health-specific sinks outside the generic scraper.
- Carbide platform code MUST keep DB/API/Vault integration, credential workflows,
  password rotation, power control, machine setup mutations, secure boot/lockdown
  mutations, and BMC user management outside the generic scraper.

## 18. Non-goals

- The generic runtime MUST NOT contain Redfish-specific discovery policy.
- The generic runtime MUST NOT contain application read models.
- The generic scraper MUST NOT perform Carbide DB writes.
- The generic scraper MUST NOT produce `EndpointExplorationReport` directly.
- The generic scraper MUST NOT require Vault or `forge_secrets`.
- The generic scraper MUST NOT mutate BMC state initially.
- The generic scraper MUST NOT perform power control initially.
- The generic scraper MUST NOT hide impossible scrape-rate configurations.
