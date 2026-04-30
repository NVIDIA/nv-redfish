use super::*;

#[test]
fn refresh_error_emits_resource_error() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.fail(
        id("/redfish/v1/Test/1"),
        FakeBmcError::Response(String::from("failed")),
    )?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let mut events = scraper.subscribe_events();

    let result = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    );
    let envelope = next_resource_event(&mut events)?;

    assert!(result.is_err());
    assert_eq!(request_counter.request_count()?, 1);
    assert!(scraper
        .inner()
        .store
        .get::<TestResource>(&id("/redfish/v1/Test/1"))
        .is_none());
    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Error { type_id, id, .. }) => {
            assert_eq!(type_id, TypeId::of::<TestResource>());
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
        }
        ScraperEvent::Resource(
            ResourceEvent::Added { .. }
            | ResourceEvent::Updated { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => {
            return Err(TestFailure::boxed(String::from("expected error event")));
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
fn events_have_monotonic_sequence_numbers() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let mut events = scraper.subscribe_events();

    let _first = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let _second = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2")),
    )?;
    let first = next_resource_event(&mut events)?;
    let second = next_resource_event(&mut events)?;

    assert!(second.seq > first.seq);
    assert!(first.timestamp <= second.timestamp);
    Ok(())
}

#[test]
fn events_are_emitted_after_store_mutation() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "stored-before-event"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let mut events = scraper.subscribe_events();

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let envelope = next_resource_event(&mut events)?;

    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Added { .. }) => {
            let stored = scraper
                .resources::<TestResource>()
                .cached(id("/redfish/v1/Test/1"))
                .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not stored")))?;
            assert_eq!(stored.value.name, "stored-before-event");
        }
        ScraperEvent::Resource(
            ResourceEvent::Updated { .. }
            | ResourceEvent::Error { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => return Err(TestFailure::boxed(String::from("expected added event"))),
        ScraperEvent::Discovery(_)
        | ScraperEvent::Relation(_)
        | ScraperEvent::Scheduler(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected resource event")))
        }
    }
    Ok(())
}

#[test]
fn new_subscriber_receives_future_events() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc.clone()).build())?;
    let _first = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let mut events = scraper.subscribe_events();
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    bmc.insert(&resource("/redfish/v1/Test/1", "second"))?;

    let _second = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let envelope = next_resource_event(&mut events)?;

    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Updated { id, .. }) => {
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
        }
        ScraperEvent::Resource(
            ResourceEvent::Added { .. }
            | ResourceEvent::Error { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => {
            return Err(TestFailure::boxed(String::from("expected updated event")));
        }
        ScraperEvent::Discovery(_)
        | ScraperEvent::Relation(_)
        | ScraperEvent::Scheduler(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected resource event")));
        }
    }
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    Ok(())
}

#[test]
fn event_stream_includes_resource_errors() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.fail(
        id("/redfish/v1/Test/1"),
        FakeBmcError::Response(String::from("failed")),
    )?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let mut events = scraper.subscribe_events();

    let result = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    );
    let envelope = next_resource_event(&mut events)?;

    assert!(result.is_err());
    assert!(scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .is_none());
    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Error { id, .. }) => {
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
        }
        ScraperEvent::Resource(
            ResourceEvent::Added { .. }
            | ResourceEvent::Updated { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => {
            return Err(TestFailure::boxed(String::from("expected error event")));
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
fn dropping_event_subscriber_does_not_stop_scraper() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "without-subscriber"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "future-subscriber"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let events = scraper.subscribe_events();
    drop(events);

    let first = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let mut future_events = scraper.subscribe_events();
    let second = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2")),
    )?;
    let envelope = next_resource_event(&mut future_events)?;

    assert_eq!(first.value.name, "without-subscriber");
    assert_eq!(second.value.name, "future-subscriber");
    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Added { id, .. }) => {
            assert_eq!(id, super::id("/redfish/v1/Test/2"));
        }
        ScraperEvent::Resource(
            ResourceEvent::Updated { .. }
            | ResourceEvent::Error { .. }
            | ResourceEvent::FreshnessMissed { .. },
        ) => return Err(TestFailure::boxed(String::from("expected added event"))),
        ScraperEvent::Discovery(_)
        | ScraperEvent::Relation(_)
        | ScraperEvent::Scheduler(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected resource event")))
        }
    }
    Ok(())
}

#[test]
fn cached_returns_none_for_unknown_resource() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let cached = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"));

    assert!(cached.is_none());
    assert_eq!(request_counter.request_count()?, 0);
    assert_eq!(scheduler_work_count(&scraper)?, 0);
    Ok(())
}

#[test]
fn cached_returns_snapshot_after_refresh() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "cached"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let refreshed = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    let cached = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not cached")))?;

    assert_eq!(cached.id, refreshed.id);
    assert_eq!(cached.value.name, refreshed.value.name);
    assert_eq!(cached.fetched_at, refreshed.fetched_at);
    assert_eq!(cached.etag, refreshed.etag);
    assert_eq!(cached.staleness, refreshed.staleness);
    Ok(())
}

#[test]
fn cached_does_not_call_bmc() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "cached"))?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let _refreshed = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let request_count = request_counter.request_count()?;
    let work_count = scheduler_work_count(&scraper)?;
    let mut events = scraper.subscribe_events();

    for _ in 0..3 {
        let cached = scraper
            .resources::<TestResource>()
            .cached(id("/redfish/v1/Test/1"))
            .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not cached")))?;
        assert_eq!(cached.value.name, "cached");
    }

    assert_eq!(request_counter.request_count()?, request_count);
    assert_eq!(scheduler_work_count(&scraper)?, work_count);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    Ok(())
}

#[test]
fn list_cached_returns_all_snapshots_for_type() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let _first = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let _second = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2")),
    )?;
    let request_count = request_counter.request_count()?;

    let all = scraper.resources::<TestResource>().list_cached();

    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, id("/redfish/v1/Test/1"));
    assert_eq!(all[0].value.name, "first");
    assert_eq!(all[1].id, id("/redfish/v1/Test/2"));
    assert_eq!(all[1].value.name, "second");
    assert_eq!(request_counter.request_count()?, request_count);
    Ok(())
}

#[test]
fn list_cached_is_type_scoped() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "test"))?;
    bmc.insert(&other_resource("/redfish/v1/Other/1", "other"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let _test = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let _other = tokio_test::block_on(
        scraper
            .resources::<OtherResource>()
            .refresh(id("/redfish/v1/Other/1")),
    )?;

    let test_resources = scraper.resources::<TestResource>().list_cached();
    let other_resources = scraper.resources::<OtherResource>().list_cached();

    assert_eq!(test_resources.len(), 1);
    assert_eq!(test_resources[0].id, id("/redfish/v1/Test/1"));
    assert_eq!(test_resources[0].value.name, "test");
    assert_eq!(other_resources.len(), 1);
    assert_eq!(other_resources[0].id, id("/redfish/v1/Other/1"));
    assert_eq!(other_resources[0].value.name, "other");
    Ok(())
}

#[test]
fn same_id_different_type_is_separate() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Shared/1", "test-value"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc.clone()).build())?;
    let _test = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Shared/1")),
    )?;
    bmc.insert(&other_resource("/redfish/v1/Shared/1", "other-value"))?;
    let _other = tokio_test::block_on(
        scraper
            .resources::<OtherResource>()
            .refresh(id("/redfish/v1/Shared/1")),
    )?;

    let test_cached = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Shared/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("test snapshot was not cached")))?;
    let other_cached = scraper
        .resources::<OtherResource>()
        .cached(id("/redfish/v1/Shared/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("other snapshot was not cached")))?;

    assert_eq!(test_cached.value.name, "test-value");
    assert_eq!(other_cached.value.name, "other-value");
    Ok(())
}

#[test]
fn cached_snapshot_arc_is_shared() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "shared"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let _refreshed = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    let first = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("first snapshot was not cached")))?;
    let second = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("second snapshot was not cached")))?;

    assert!(Arc::ptr_eq(&first.value, &second.value));
    Ok(())
}
