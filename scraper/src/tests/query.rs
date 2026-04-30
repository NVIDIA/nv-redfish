use super::*;

#[test]
fn list_uses_registered_discoverer() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let discoverer = TestDiscoverer::new(Vec::new());
    let invoked = discoverer.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(discoverer))
            .build(),
    )?;

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;

    assert!(resources.is_empty());
    assert_eq!(invoked.invoked_count(), 1);
    Ok(())
}

#[test]
fn list_fetches_discovered_candidates() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    let request_counter = bmc.clone();
    let discoverer = TestDiscoverer::new(vec![id("/redfish/v1/Test/1"), id("/redfish/v1/Test/2")]);
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(discoverer))
            .build(),
    )?;

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;

    assert_eq!(resources.len(), 2);
    assert_eq!(request_counter.request_count()?, 2);
    assert_eq!(scraper.inner().scheduler.records()?.len(), 2);
    Ok(())
}

#[test]
fn raw_resources_refresh_unknown_oem_json() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1/Oem/Nvidia/Thing"),
        json!({
            "@odata.id": "/redfish/v1/Oem/Nvidia/Thing",
            "@odata.etag": "raw-etag",
            "VendorValue": 42,
        }),
    )?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let snapshot = tokio_test::block_on(
        scraper
            .raw_resources()
            .refresh(id("/redfish/v1/Oem/Nvidia/Thing")),
    )?;

    assert_eq!(snapshot.id, id("/redfish/v1/Oem/Nvidia/Thing"));
    assert_eq!(snapshot.etag, Some(etag("raw-etag")));
    assert_eq!(snapshot.value.value()["VendorValue"], json!(42));
    assert!(scraper
        .resources::<RawResource>()
        .cached(id("/redfish/v1/Oem/Nvidia/Thing"))
        .is_some());
    Ok(())
}

#[test]
fn manual_discoverer_can_fetch_raw_json_through_scheduler() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1/Oem/Nvidia/TestCollection"),
        json!({
            "@odata.id": "/redfish/v1/Oem/Nvidia/TestCollection",
            "Members": [
                { "@odata.id": "/redfish/v1/Test/1" },
            ],
        }),
    )?;
    bmc.insert(&resource("/redfish/v1/Test/1", "from-raw-discovery"))?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(RawDiscoverer::new(
                "/redfish/v1/Oem/Nvidia/TestCollection",
            )))
            .build(),
    )?;
    let mut events = scraper.subscribe_events();

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;
    let discovery = next_discovery_event(&mut events)?;
    let ScraperEvent::Discovery(DiscoveryEvent::Candidates { source_id, .. }) = discovery.event
    else {
        return Err(TestFailure::boxed(String::from("expected discovery event")));
    };

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].value.name, "from-raw-discovery");
    assert_eq!(
        request_counter.requested_ids()?[0],
        id("/redfish/v1/Oem/Nvidia/TestCollection")
    );
    assert_eq!(scraper.inner().scheduler.records()?.len(), 2);
    assert_eq!(
        scraper.inner().store.discovery_source_members(source_id)?,
        vec![ResourceRef::of::<TestResource>(id("/redfish/v1/Test/1"))]
            .into_iter()
            .collect()
    );
    Ok(())
}

#[test]
fn query_plan_and_store_membership_are_recorded() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "member"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;
    let mut events = scraper.subscribe_events();

    let subscription = tokio_test::block_on(scraper.query::<TestResource>().subscribe())?;
    let registered = next_query_event(&mut events)?;
    let query_id = match registered.event {
        ScraperEvent::Query(QueryEvent::Registered { id, kind, type_id }) => {
            assert_eq!(kind, QueryKind::LongLived);
            assert_eq!(type_id, TypeId::of::<TestResource>());
            id
        }
        _ => {
            return Err(TestFailure::boxed(String::from(
                "expected query registration",
            )))
        }
    };

    let expected = vec![ResourceRef::of::<TestResource>(id("/redfish/v1/Test/1"))]
        .into_iter()
        .collect();
    let plan = scraper
        .inner()
        .queries
        .plan(query_id)?
        .ok_or_else(|| TestFailure::boxed(String::from("missing query plan")))?;
    assert_eq!(plan.members, expected);
    assert_eq!(scraper.inner().store.query_members(query_id)?, expected);

    drop(subscription);
    let removed = next_query_event(&mut events)?;
    assert!(matches!(
        removed.event,
        ScraperEvent::Query(QueryEvent::Removed { id, .. }) if id == query_id
    ));
    assert!(scraper.inner().queries.plan(query_id)?.is_none());
    assert!(scraper.inner().store.query_members(query_id)?.is_empty());
    Ok(())
}

#[test]
fn list_returns_matching_snapshots() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "listed"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, id("/redfish/v1/Test/1"));
    assert_eq!(resources[0].value.name, "listed");
    let cached = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not stored")))?;
    assert_eq!(cached.value.name, "listed");
    Ok(())
}

#[test]
fn list_removes_temporary_demand_after_return() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                Vec::new(),
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;

    assert!(resources.is_empty());
    assert_eq!(scraper.inner().queries.active_temporary()?, 0);
    Ok(())
}

#[test]
fn list_emits_discovered_and_added_events() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "evented"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;
    let mut events = scraper.subscribe_events();

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;
    let discovery = next_discovery_event(&mut events)?;
    let resource = next_resource_event(&mut events)?;

    assert_eq!(resources.len(), 1);
    assert!(discovery.seq < resource.seq);
    match discovery.event {
        ScraperEvent::Discovery(DiscoveryEvent::Candidates { type_id, ids, .. }) => {
            assert_eq!(type_id, TypeId::of::<TestResource>());
            assert_eq!(ids, vec![id("/redfish/v1/Test/1")]);
        }
        ScraperEvent::Relation(_)
        | ScraperEvent::Resource(_)
        | ScraperEvent::Scheduler(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected discovery event")));
        }
    }
    match resource.event {
        ScraperEvent::Resource(ResourceEvent::Added { id, .. }) => {
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
        }
        ScraperEvent::Resource(
            ResourceEvent::Updated { .. }
            | ResourceEvent::Error { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => {
            return Err(TestFailure::boxed(String::from("expected added event")));
        }
        ScraperEvent::Discovery(_)
        | ScraperEvent::Relation(_)
        | ScraperEvent::Scheduler(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected resource event")));
        }
    }
    Ok(())
}

#[test]
fn list_with_no_discoverer_returns_empty() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let resources = tokio_test::block_on(scraper.query::<TestResource>().list())?;

    assert!(resources.is_empty());
    assert_eq!(request_counter.request_count()?, 0);
    assert_eq!(scraper.inner().queries.active_temporary()?, 0);
    Ok(())
}

#[test]
fn list_applies_snapshot_predicate() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "match"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "skip"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1"), id("/redfish/v1/Test/2")],
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(NameContainsPredicate::new("match"))
            .list(),
    )?;

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, id("/redfish/v1/Test/1"));
    assert_eq!(resources[0].value.name, "match");
    Ok(())
}

#[test]
fn predicate_can_filter_by_resource_id() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Inlet", "inlet"))?;
    bmc.insert(&resource("/redfish/v1/Test/Outlet", "outlet"))?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/Inlet"), id("/redfish/v1/Test/Outlet")],
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::id().contains("Inlet"))
            .list(),
    )?;

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, id("/redfish/v1/Test/Inlet"));
    assert_eq!(request_counter.request_count()?, 1);
    Ok(())
}

#[test]
fn sensor_predicates_filter_by_reading_type_context_and_name() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(id("/redfish/v1"), service_root("/redfish/v1/Chassis"))?;
    bmc.insert_value(
        id("/redfish/v1/Chassis"),
        chassis_collection("/redfish/v1/Chassis", &["/redfish/v1/Chassis/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1"),
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            None,
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/Sensors"),
        sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &[
                "/redfish/v1/Chassis/1/Sensors/GpuTemp",
                "/redfish/v1/Chassis/1/Sensors/InletTemp",
            ],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/Sensors/GpuTemp"),
        redfish_sensor_with_context("/redfish/v1/Chassis/1/Sensors/GpuTemp", "GPU Temp", "GPU"),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/Sensors/InletTemp"),
        redfish_sensor_with_context(
            "/redfish/v1/Chassis/1/Sensors/InletTemp",
            "Inlet Temp",
            "Intake",
        ),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(
        scraper
            .query::<RedfishSensor>()
            .where_(sensor_predicate::reading_type().equals(ReadingType::Temperature))
            .where_(sensor_predicate::physical_context().equals(PhysicalContext::Gpu))
            .where_(sensor_predicate::name().contains("GPU"))
            .list(),
    )?;

    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].id, id("/redfish/v1/Chassis/1/Sensors/GpuTemp"));
    Ok(())
}

#[test]
fn sensor_predicate_hints_remain_discovery_only() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(id("/redfish/v1"), service_root("/redfish/v1/Chassis"))?;
    bmc.insert_value(
        id("/redfish/v1/Chassis"),
        chassis_collection("/redfish/v1/Chassis", &["/redfish/v1/Chassis/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1"),
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            None,
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/Sensors"),
        sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &["/redfish/v1/Chassis/1/Sensors/Fan"],
        ),
    )?;
    let mut fan = redfish_sensor("/redfish/v1/Chassis/1/Sensors/Fan", "Fan");
    fan["ReadingType"] = json!("Fan");
    bmc.insert_value(id("/redfish/v1/Chassis/1/Sensors/Fan"), fan)?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(
        scraper
            .query::<RedfishSensor>()
            .where_(sensor_predicate::reading_type().equals(ReadingType::Temperature))
            .list(),
    )?;

    assert!(sensors.is_empty());
    Ok(())
}

#[test]
fn multiple_predicates_are_and_combined() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Inlet", "temperature"))?;
    bmc.insert(&resource("/redfish/v1/Test/InletVoltage", "voltage"))?;
    bmc.insert(&resource("/redfish/v1/Test/Outlet", "temperature"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![
                    id("/redfish/v1/Test/Inlet"),
                    id("/redfish/v1/Test/InletVoltage"),
                    id("/redfish/v1/Test/Outlet"),
                ],
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::id().contains("Inlet"))
            .where_(NameContainsPredicate::new("temperature"))
            .list(),
    )?;

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, id("/redfish/v1/Test/Inlet"));
    assert_eq!(resources[0].value.name, "temperature");
    Ok(())
}

#[test]
fn predicate_failure_does_not_fetch_unneeded_candidates_when_candidate_stage_applies(
) -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Inlet", "inlet"))?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/Inlet"), id("/redfish/v1/Test/Missing")],
            )))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::id().contains("Inlet"))
            .list(),
    )?;

    assert_eq!(resources.len(), 1);
    assert_eq!(request_counter.request_count()?, 1);
    Ok(())
}

#[test]
fn predicate_hints_are_passed_to_discoverer() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let discoverer = TestDiscoverer::new(Vec::new());
    let recorded = discoverer.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(discoverer))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::id().contains("Inlet"))
            .where_(NameContainsPredicate::with_hint(
                "temperature",
                DiscoveryHint::semantic("reading_type:temperature"),
            ))
            .list(),
    )?;

    assert!(resources.is_empty());
    let hints = recorded.hints()?;
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].id_contains_hints(), &[String::from("Inlet")]);
    assert_eq!(
        hints[0].semantic_hints(),
        &[String::from("reading_type:temperature")]
    );
    Ok(())
}
