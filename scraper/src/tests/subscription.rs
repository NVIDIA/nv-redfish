use super::*;

#[test]
fn subscribe_runs_initial_list() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let request_counter = bmc.clone();
    let discoverer = TestDiscoverer::new(vec![id("/redfish/v1/Test/1")]);
    let invoked = discoverer.clone();
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(discoverer))
            .build(),
    )?;

    let subscription = tokio_test::block_on(scraper.query::<TestResource>().subscribe())?;

    assert_eq!(invoked.invoked_count(), 1);
    assert_eq!(request_counter.request_count()?, 1);
    assert_eq!(scraper.inner().queries.active_long_lived()?, 1);
    drop(subscription);
    Ok(())
}

#[test]
fn subscribe_emits_added_for_initial_matches() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;
    let mut subscription = tokio_test::block_on(scraper.query::<TestResource>().subscribe())?;

    let event = tokio_test::block_on(subscription.recv())?;

    match event {
        TypedResourceEvent::Added(snapshot) => {
            assert_eq!(snapshot.id, id("/redfish/v1/Test/1"));
            assert_eq!(snapshot.value.name, "initial");
        }
        TypedResourceEvent::Updated { .. }
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected added event")));
        }
    }
    Ok(())
}

#[test]
fn subscribe_filters_global_events_by_query() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Match", "initial"))?;
    bmc.insert(&resource("/redfish/v1/Test/Skip", "initial"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc.clone())
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/Match"), id("/redfish/v1/Test/Skip")],
            )))
            .build(),
    )?;
    let mut subscription = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::id().contains("Match"))
            .subscribe(),
    )?;
    let _initial = tokio_test::block_on(subscription.recv())?;
    bmc.insert(&resource("/redfish/v1/Test/Skip", "skip-updated"))?;
    let _skipped = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/Skip")),
    )?;
    bmc.insert(&resource("/redfish/v1/Test/Match", "match-updated"))?;

    let _matched = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/Match")),
    )?;
    let event = tokio_test::block_on(subscription.recv())?;

    match event {
        TypedResourceEvent::Updated { new, .. } => {
            assert_eq!(new.id, id("/redfish/v1/Test/Match"));
            assert_eq!(new.value.name, "match-updated");
        }
        TypedResourceEvent::Added(_)
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected updated event")));
        }
    }
    Ok(())
}

#[test]
fn subscribe_emits_updated_for_matching_resource() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "match-a"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc.clone())
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;
    let mut subscription = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(NameContainsPredicate::new("match"))
            .subscribe(),
    )?;
    let _initial = tokio_test::block_on(subscription.recv())?;
    bmc.insert(&resource("/redfish/v1/Test/1", "match-b"))?;

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let event = tokio_test::block_on(subscription.recv())?;

    match event {
        TypedResourceEvent::Updated { new, .. } => {
            assert_eq!(new.id, id("/redfish/v1/Test/1"));
            assert_eq!(new.value.name, "match-b");
        }
        TypedResourceEvent::Added(_)
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected updated event")));
        }
    }
    Ok(())
}

#[test]
fn subscribe_emits_removed_when_resource_no_longer_matches() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "match"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc.clone())
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;
    let mut subscription = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(NameContainsPredicate::new("match"))
            .subscribe(),
    )?;
    let _initial = tokio_test::block_on(subscription.recv())?;
    bmc.insert(&resource("/redfish/v1/Test/1", "skip"))?;

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let event = tokio_test::block_on(subscription.recv())?;

    match event {
        TypedResourceEvent::Removed(id) => {
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
        }
        TypedResourceEvent::Added(_)
        | TypedResourceEvent::Updated { .. }
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected removed event")));
        }
    }
    Ok(())
}

#[test]
fn dropping_subscription_removes_query_demand() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
                vec![id("/redfish/v1/Test/1")],
            )))
            .build(),
    )?;

    let subscription = tokio_test::block_on(scraper.query::<TestResource>().subscribe())?;
    assert_eq!(scraper.inner().queries.active_long_lived()?, 1);
    drop(subscription);

    assert_eq!(scraper.inner().queries.active_long_lived()?, 0);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn subscribe_refreshes_matching_resource_when_stale() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let request_counter = bmc.clone();
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
            vec![id("/redfish/v1/Test/1")],
        )))
        .build()
        .await?;
    let mut subscription = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .subscribe()
        .await?;
    let _initial = subscription.recv().await?;
    bmc.insert(&resource("/redfish/v1/Test/1", "refreshed"))?;

    advance(Duration::from_secs(5)).await;
    wait_for_recorded_requests(&request_counter, 2).await?;
    let event = subscription.recv().await?;
    match event {
        TypedResourceEvent::FreshnessMissed { id, desired, .. } => {
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
            assert_eq!(desired, Duration::from_secs(5));
        }
        TypedResourceEvent::Added(_)
        | TypedResourceEvent::Updated { .. }
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from(
                "expected freshness missed event",
            )));
        }
    }
    let event = subscription.recv().await?;

    match event {
        TypedResourceEvent::Updated { new, .. } => {
            assert_eq!(new.id, id("/redfish/v1/Test/1"));
            assert_eq!(new.value.name, "refreshed");
        }
        TypedResourceEvent::Added(_)
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected updated event")));
        }
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn watch_refreshes_without_returning_typed_events() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let request_counter = bmc.clone();
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
            vec![id("/redfish/v1/Test/1")],
        )))
        .build()
        .await?;
    let _watch = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .watch()
        .await?;
    bmc.insert(&resource("/redfish/v1/Test/1", "watched"))?;

    advance(Duration::from_secs(5)).await;
    wait_for_recorded_requests(&request_counter, 2).await?;
    let cached = scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not cached")))?;

    assert_eq!(cached.value.name, "watched");
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn dropping_watch_stops_background_demand() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let request_counter = bmc.clone();
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
            vec![id("/redfish/v1/Test/1")],
        )))
        .build()
        .await?;
    let watch = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .watch()
        .await?;
    assert_eq!(scraper.inner().queries.active_long_lived()?, 1);
    drop(watch);
    bmc.insert(&resource("/redfish/v1/Test/1", "after-drop"))?;

    advance(Duration::from_secs(20)).await;
    yield_now().await;

    assert_eq!(request_counter.request_count()?, 1);
    assert_eq!(scraper.inner().queries.active_long_lived()?, 0);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn resource_freshness_and_discovery_freshness_are_independent(
) -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    let request_counter = bmc.clone();
    let discoverer = TestDiscoverer::new(vec![id("/redfish/v1/Test/1")]);
    let invoked = discoverer.clone();
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(discoverer))
        .build()
        .await?;
    let _watch = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(30))
        .discovery_freshness(Duration::from_secs(20))
        .watch()
        .await?;

    advance(Duration::from_secs(20)).await;
    for _ in 0..8 {
        if invoked.invoked_count() >= 2 {
            break;
        }
        yield_now().await;
    }
    assert_eq!(invoked.invoked_count(), 2);
    assert_eq!(request_counter.request_count()?, 1);
    bmc.insert(&resource("/redfish/v1/Test/1", "fresh-resource"))?;

    advance(Duration::from_secs(10)).await;
    wait_for_recorded_requests(&request_counter, 2).await?;

    assert_eq!(invoked.invoked_count(), 2);
    assert_eq!(request_counter.request_count()?, 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn stale_snapshot_reports_age_and_desired_freshness() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "cached"))?;
    let request_counter = bmc.clone();
    let scraper = Scraper::builder(bmc).build().await?;
    let _snapshot = scraper
        .resources::<TestResource>()
        .refresh(id("/redfish/v1/Test/1"))
        .await?;

    advance(Duration::from_secs(6)).await;
    let cached = scraper
        .resources::<TestResource>()
        .cached_with_freshness(id("/redfish/v1/Test/1"), Duration::from_secs(5))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not cached")))?;

    match cached.staleness {
        Staleness::Stale { age, desired } => {
            assert!(age >= Duration::from_secs(6));
            assert_eq!(desired, Some(Duration::from_secs(5)));
        }
        Staleness::Fresh => {
            return Err(TestFailure::boxed(String::from("expected stale snapshot")));
        }
    }
    assert_eq!(request_counter.request_count()?, 1);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn missed_poll_ticks_do_not_accumulate() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    bmc.release_all();
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
            vec![id("/redfish/v1/Test/1")],
        )))
        .build()
        .await?;
    let _watch = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .watch()
        .await?;
    bmc.block_all();

    advance(Duration::from_secs(50)).await;
    bmc.wait_for_in_flight(1).await?;
    assert_eq!(bmc.request_count()?, 2);

    advance(Duration::from_secs(50)).await;
    yield_now().await;
    assert_eq!(bmc.request_count()?, 2);
    bmc.release_all();
    wait_for_blocking_request_count(&bmc, 2).await?;
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn resource_has_at_most_one_pending_refresh() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "initial"))?;
    bmc.release_all();
    let discoverer = TestDiscoverer::new(vec![id("/redfish/v1/Test/1")]);
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .discover(Discovery::manual::<TestResource, _>(discoverer))
        .build()
        .await?;
    let _first = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .watch()
        .await?;
    let _second = scraper
        .query::<TestResource>()
        .freshness(Duration::from_secs(5))
        .watch()
        .await?;
    assert_eq!(bmc.request_count()?, 2);
    bmc.block_all();

    advance(Duration::from_secs(5)).await;
    bmc.wait_for_in_flight(1).await?;
    yield_now().await;

    assert_eq!(bmc.request_count()?, 3);
    bmc.release_all();
    Ok(())
}
