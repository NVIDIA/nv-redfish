# Redfish adapter

The Redfish adapter binds the generic scraper runtime to `nv-redfish`.

It is not the application policy layer. It provides feature-gated generator
builders and Redfish work event types that applications can use with the runtime.

## Scope

The adapter provides:

- Redfish work event types,
- feature-gated generator builders,
- helpers that close over `nv-redfish` objects,
- conversion from fetched schema data to work events,
- preservation of Redfish identity, parent identity, metadata, and expanded data,
- optional reconstruction helpers.

The adapter depends on `nv-redfish`. The generic runtime does not.

## Feature gating

Adapter features follow scraper and `nv-redfish` feature boundaries.

If a capability feature is disabled:

- related generator builders are not compiled,
- related work event payload variants are not compiled because the generated
  schema type is not compiled,
- related fetch logic is not compiled,
- applications cannot request that capability through adapter types.

Runtime configuration still decides which compiled generators are actually added
to the scheduler tree.

## Generated `EntityPayload`

The `nv-redfish` CSDL compiler should optionally generate an `EntityPayload` enum
with one variant per compiled entity type.

Conceptual shape:

```rust
pub enum EntityPayload {
    ServiceRoot(Arc<schema::service_root::ServiceRoot>),
    Chassis(Arc<schema::chassis::Chassis>),
    ChassisCollection(Arc<schema::chassis_collection::ChassisCollection>),
    ComputerSystem(Arc<schema::computer_system::ComputerSystem>),
    Sensor(Arc<schema::sensor::Sensor>),
    // one variant for each compiled entity type
}
```

Generated support should expose:

- entity kind,
- `@odata.id`,
- `@odata.etag`, when present.

The adapter uses `EntityPayload` in work events. It does not copy Redfish data
into a parallel domain model.

## Serialization support

Distributed scraping and event forwarding are application concerns, but the
adapter must not prevent them.

The `nv-redfish` CSDL compiler should support a feature or codegen flag that
adds `serde::Serialize` derives to generated read/entity data. With that enabled,
Redfish work events can serialize `EntityPayload` plus metadata and transfer it
to another node.

Serialized Redfish work events contain read-side data:

- `BmcId`,
- `ODataId`,
- optional parent `ODataId`,
- change metadata,
- `EntityPayload`,
- scrape metadata and errors.

Serialized events do not contain execution handles such as `Chassis<B>`,
`ComputerSystem<B>`, sensor links, or `Bmc` clients.

## Redfish work events

The adapter defines the Redfish work event type used as runtime work event `E`.
Successful Redfish work returns one or more `E` values; the runtime wraps them in
`RuntimeOutput::Work(Ok(WorkSuccess<E>))`. Failed Redfish work returns an adapter
or application error `Err`; the runtime wraps it in
`RuntimeOutput::Work(Err(WorkError<Err>))`.

```rust
pub struct RedfishResourceEvent {
    pub bmc_id: BmcId,
    pub odata_id: ODataId,
    pub parent_odata_id: Option<ODataId>,
    pub change: ChangeKind,
    pub payload: Option<EntityPayload>,
    pub metadata: ResourceMetadata,
}

pub enum RedfishEvent {
    Resource(RedfishResourceEvent),
    Generator(GeneratorEvent),
    Scrape(ScrapeEvent),
}
```

Scheduler, executor, queue, lag, and throttling facts are runtime events, not
Redfish work events.

`parent_odata_id` is used when the adapter or application knows the resource's
parent. It supports hierarchy reconstruction and application projections.

Work events do not contain `Chassis<B>`, `ComputerSystem<B>`, or other execution
handles.

## Generators and nv-redfish objects

Adapter generators close over valid `nv-redfish` objects and call valid methods
on those objects.

Examples:

- a service-root generator closes over `ServiceRoot<B>`,
- a chassis collection generator may close over `ServiceRoot<B>` or a collection
  wrapper,
- a chassis inventory generator closes over `Chassis<B>`,
- a chassis sensor generator closes over `Chassis<B>` or sensor links,
- a system generator closes over `ComputerSystem<B>`,
- a firmware generator closes over update-service or firmware inventory objects.

This avoids a detached command language such as "fetch sensors from arbitrary
object". The object type determines which operations are valid.

## Application-driven discovery

The adapter does not decide the discovery plan.

An application may:

1. create `ServiceRoot<B>`,
2. add a service-root generator,
3. consume a service-root work output,
4. inspect its own config and model,
5. add chassis, systems, managers, firmware, sensor, or OEM generators as needed.

Different applications can therefore use different discovery breadth without
changing the runtime.

## Expand handling

`nv-redfish` represents navigation properties as `NavProperty<T>`.

A navigation property can be:

- a reference containing only `@odata.id`, or
- an expanded value containing `Arc<T>`.

`nv-redfish` also supports `ExpandQuery` through the `Bmc` trait and high-level
wrapper helpers. Expand can reduce request count by returning inline related
resources.

Adapter rules:

- expanded payloads must not be collapsed to links only,
- if a fetched entity contains expanded `NavProperty<T>`, the emitted
  `EntityPayload` preserves that expanded data,
- the adapter may also emit separate child resource work events for expanded
  children,
- child work events should include `parent_odata_id` when the parent is known,
- expand choice is part of the generator/helper policy, not the generic runtime.

This allows applications to retain both the original Redfish structure and a
resource work-event view.

## Optional reconstruction helpers

The adapter may provide optional reconstruction helpers.

Responsibilities:

- rebuild hierarchy from `odata_id` and `parent_odata_id`,
- rebuild adapter generators from stored resource work events or records,
- reconstruct `nv-redfish` wrapper objects from stored schema payloads plus a
  live `Bmc`, when `nv-redfish` exposes suitable constructors.

Reconstruction is not required for consuming work events. It is only needed if an
application wants to restore scraper execution state without full rediscovery.

## User-owned behavior

The adapter does not provide:

- Carbide `to_model()` conversion,
- `EndpointExplorationReport`,
- health metric naming,
- DB/API/Vault integration,
- password rotation,
- power control,
- secure boot or machine setup mutations,
- BMC user management.
