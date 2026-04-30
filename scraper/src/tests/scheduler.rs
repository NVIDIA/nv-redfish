use super::*;

#[test]
fn refresh_fetches_known_resource() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let response = resource("/redfish/v1/Test/1", "version-a");
    bmc.insert(&response)?;
    let request_counter = bmc.clone();
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    assert_eq!(snapshot.id, id("/redfish/v1/Test/1"));
    assert_eq!(snapshot.value.name, "version-a");
    assert_eq!(snapshot.staleness, Staleness::Fresh);
    assert_eq!(request_counter.request_count()?, 1);
    Ok(())
}

#[test]
fn refresh_stores_snapshot() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "stored"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    let stored = scraper
        .inner()
        .store
        .get::<TestResource>(&id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not stored")))?;
    assert_eq!(stored.value.name, "stored");
    Ok(())
}

#[test]
fn refresh_emits_resource_added_event() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "added"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let mut events = scraper.subscribe_events();

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let envelope = next_resource_event(&mut events)?;

    assert!(envelope.seq.as_u64() > 0);
    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Added { type_id, id }) => {
            assert_eq!(type_id, TypeId::of::<TestResource>());
            assert_eq!(id, super::id("/redfish/v1/Test/1"));
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
    let stored = scraper
        .inner()
        .store
        .get::<TestResource>(&id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not stored")))?;
    assert_eq!(stored.value.name, "added");
    Ok(())
}

#[test]
fn refresh_emits_resource_updated_event_on_second_value() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "version-a"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc.clone()).build())?;

    let _first = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    bmc.insert(&resource("/redfish/v1/Test/1", "version-b"))?;
    let mut events = scraper.subscribe_events();

    let _second = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let envelope = next_resource_event(&mut events)?;

    match envelope.event {
        ScraperEvent::Resource(ResourceEvent::Updated { type_id, id }) => {
            assert_eq!(type_id, TypeId::of::<TestResource>());
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
    let stored = scraper
        .inner()
        .store
        .get::<TestResource>(&id("/redfish/v1/Test/1"))
        .ok_or_else(|| TestFailure::boxed(String::from("snapshot was not stored")))?;
    assert_eq!(stored.value.name, "version-b");
    Ok(())
}

#[test]
fn refresh_uses_scheduler() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "scheduled"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    let records = scraper.inner().scheduler.records()?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lane, Lane::Interactive);
    assert_eq!(records[0].operation, Operation::Get);
    assert_eq!(records[0].id, id("/redfish/v1/Test/1"));
    assert_eq!(records[0].type_id, TypeId::of::<TestResource>());
    Ok(())
}

#[tokio::test]
async fn scheduler_limits_in_flight_requests() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    for index in 1..=4 {
        let path = format!("/redfish/v1/Test/{index}");
        bmc.insert(&resource(&path, &format!("resource-{index}")))?;
    }
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(2)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let mut handles = Vec::new();
    for index in 1..=4 {
        let scraper = scraper.clone();
        let resource_id = id(&format!("/redfish/v1/Test/{index}"));
        handles.push(tokio::spawn(async move {
            scraper
                .resources::<TestResource>()
                .refresh(resource_id)
                .await
        }));
    }

    bmc.wait_for_in_flight(2).await?;
    assert_eq!(bmc.current_in_flight()?, 2);
    assert_eq!(bmc.max_in_flight()?, 2);
    bmc.release_all();

    for handle in handles {
        let _snapshot = handle.await??;
    }
    assert_eq!(bmc.max_in_flight()?, 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn scheduler_limits_request_rate() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    let request_counter = bmc.clone();
    let scraper = Scraper::builder(bmc)
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(2)
                .max_requests_per_second(1),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    yield_now().await;
    assert_eq!(request_counter.request_count()?, 1);

    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2"))
            .await
    });
    yield_now().await;
    assert_eq!(request_counter.request_count()?, 1);

    advance(Duration::from_millis(999)).await;
    yield_now().await;
    assert_eq!(request_counter.request_count()?, 1);

    advance(Duration::from_millis(1)).await;
    let _first = first.await??;
    let _second = second.await??;
    assert_eq!(request_counter.request_count()?, 2);
    Ok(())
}

#[tokio::test]
async fn adaptive_scheduler_starts_conservative() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::adaptive()
                .initial_in_flight(1)
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2"))
            .await
    });

    bmc.wait_for_in_flight(1).await?;
    yield_now().await;
    assert_eq!(bmc.current_in_flight()?, 1);
    assert_eq!(bmc.request_count()?, 1);

    bmc.release_all();
    let _first = first.await??;
    let _second = second.await??;
    Ok(())
}

#[tokio::test]
async fn adaptive_scheduler_increases_after_healthy_window() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    for index in 1..=4 {
        bmc.insert(&resource(
            &format!("/redfish/v1/Test/{index}"),
            &format!("resource-{index}"),
        ))?;
    }
    let scraper = Scraper::builder(bmc)
        .capacity(
            BmcCapacity::adaptive()
                .initial_in_flight(1)
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;
    let mut events = scraper.subscribe_events();

    for index in 1..=4 {
        let _snapshot = scraper
            .resources::<TestResource>()
            .refresh(id(&format!("/redfish/v1/Test/{index}")))
            .await?;
    }

    let stats = drain_scheduler_stats(&mut events)?;
    assert!(stats.iter().any(|state| state.in_flight_limit == 2));
    Ok(())
}

#[tokio::test]
async fn adaptive_scheduler_decreases_after_503() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.fail(
        id("/redfish/v1/Test/1"),
        FakeBmcError::Response(String::from("503 overloaded")),
    )?;
    let scraper = Scraper::builder(bmc)
        .capacity(
            BmcCapacity::adaptive()
                .initial_in_flight(4)
                .max_in_flight(8)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;
    let mut events = scraper.subscribe_events();

    let result = scraper
        .resources::<TestResource>()
        .refresh(id("/redfish/v1/Test/1"))
        .await;

    assert!(result.is_err());
    let stats = drain_scheduler_stats(&mut events)?;
    assert!(stats
        .iter()
        .any(|state| { state.in_flight_limit == 2 && state.load_state == LoadState::Overloaded }));
    Ok(())
}

#[tokio::test]
async fn interactive_refresh_still_respects_adaptive_hard_limits() -> Result<(), Box<dyn StdError>>
{
    let bmc = BlockingBmc::default();
    for index in 1..=4 {
        bmc.insert(&resource(
            &format!("/redfish/v1/Test/{index}"),
            &format!("resource-{index}"),
        ))?;
    }
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::adaptive()
                .initial_in_flight(2)
                .max_in_flight(2)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let mut handles = Vec::new();
    for index in 1..=4 {
        let scraper = scraper.clone();
        handles.push(tokio::spawn(async move {
            scraper
                .resources::<TestResource>()
                .refresh(id(&format!("/redfish/v1/Test/{index}")))
                .await
        }));
    }

    bmc.wait_for_in_flight(2).await?;
    yield_now().await;
    assert_eq!(bmc.current_in_flight()?, 2);
    assert_eq!(bmc.max_in_flight()?, 2);

    bmc.release_all();
    for handle in handles {
        let _snapshot = handle.await??;
    }
    assert_eq!(bmc.max_in_flight()?, 2);
    Ok(())
}

#[tokio::test]
async fn scheduler_records_lane_for_each_request() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let lanes = [
        Lane::Interactive,
        Lane::Subscription,
        Lane::Discovery,
        Lane::Maintenance,
    ];
    for (index, _lane) in lanes.iter().enumerate() {
        let path = format!("/redfish/v1/Test/{index}");
        bmc.insert(&resource(&path, &format!("lane-{index}")))?;
    }
    let scraper = Scraper::builder(bmc).build().await?;

    for (index, lane) in lanes.iter().copied().enumerate() {
        let path = id(&format!("/redfish/v1/Test/{index}"));
        let _snapshot = scraper
            .inner()
            .scheduler
            .get_for_lane::<_, TestResource>(
                &scraper.inner().bmc,
                &scraper.inner().events,
                lane,
                path,
            )
            .await?;
    }

    let records = scraper.inner().scheduler.records()?;
    assert_eq!(records.len(), lanes.len());
    for (record, lane) in records.iter().zip(lanes) {
        assert_eq!(record.lane, lane);
        assert_eq!(record.operation, Operation::Get);
    }
    Ok(())
}

#[test]
fn interactive_request_completes_through_scheduler() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "interactive"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .capacity(
                BmcCapacity::fixed()
                    .max_in_flight(4)
                    .max_requests_per_second(10),
            )
            .build(),
    )?;

    let snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;

    let records = scraper.inner().scheduler.records()?;
    assert_eq!(snapshot.value.name, "interactive");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lane, Lane::Interactive);
    assert_eq!(records[0].operation, Operation::Get);
    Ok(())
}

#[test]
fn scheduler_emits_basic_stats_event() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "stats"))?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .capacity(
                BmcCapacity::fixed()
                    .max_in_flight(1)
                    .max_requests_per_second(10),
            )
            .build(),
    )?;
    let mut events = scraper.subscribe_events();

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1")),
    )?;
    let envelope = next_scheduler_event(&mut events)?;

    match envelope.event {
        ScraperEvent::Scheduler(SchedulerEvent::Stats { state }) => {
            assert_eq!(state.queued, 1);
            assert_eq!(state.in_flight, 0);
        }
        ScraperEvent::Scheduler(SchedulerEvent::LoadChanged { .. })
        | ScraperEvent::Relation(_)
        | ScraperEvent::Resource(_)
        | ScraperEvent::Discovery(_)
        | ScraperEvent::Query(_) => {
            return Err(TestFailure::boxed(String::from("expected scheduler event")));
        }
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_refresh_same_resource_uses_one_bmc_request() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "coalesced"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    yield_now().await;

    assert_eq!(bmc.request_count()?, 1);
    bmc.release_all();
    let first = first.await??;
    let second = second.await??;

    assert_eq!(first.value.name, "coalesced");
    assert_eq!(second.value.name, "coalesced");
    assert_eq!(bmc.request_count()?, 1);
    Ok(())
}

#[tokio::test]
async fn coalesced_waiters_receive_same_snapshot() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "same-snapshot"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    yield_now().await;
    bmc.release_all();

    let first = first.await??;
    let second = second.await??;
    assert_eq!(first.id, second.id);
    assert_eq!(first.value.name, second.value.name);
    assert_eq!(first.etag, second.etag);
    assert_eq!(first.fetched_at, second.fetched_at);
    assert_eq!(first.staleness, second.staleness);
    assert!(Arc::ptr_eq(&first.value, &second.value));
    Ok(())
}

#[tokio::test]
async fn coalesced_error_is_returned_to_all_waiters() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.fail(
        id("/redfish/v1/Test/1"),
        FakeBmcError::Response(String::from("failed")),
    )?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;
    let mut events = scraper.subscribe_events();

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    yield_now().await;
    bmc.release_all();

    let first = first.await?;
    let second = second.await?;
    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(bmc.request_count()?, 1);
    assert!(scraper
        .resources::<TestResource>()
        .cached(id("/redfish/v1/Test/1"))
        .is_none());
    let resource_events = drain_resource_events(&mut events)?;
    assert_eq!(resource_events.len(), 1);
    assert!(matches!(resource_events[0], ResourceEvent::Error { .. }));
    Ok(())
}

#[tokio::test]
async fn different_types_or_ids_do_not_coalesce() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    bmc.insert(&resource("/redfish/v1/Test/2", "second"))?;
    bmc.insert(&resource("/redfish/v1/Shared/1", "shared-test"))?;
    bmc.insert(&other_resource("/redfish/v1/Shared/1", "other"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    let second_scraper = scraper.clone();
    let second = tokio::spawn(async move {
        second_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/2"))
            .await
    });
    let third_scraper = scraper.clone();
    let third = tokio::spawn(async move {
        third_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Shared/1"))
            .await
    });
    let fourth_scraper = scraper.clone();
    let fourth = tokio::spawn(async move {
        fourth_scraper
            .resources::<OtherResource>()
            .refresh(id("/redfish/v1/Shared/1"))
            .await
    });

    bmc.wait_for_in_flight(4).await?;
    assert_eq!(bmc.request_count()?, 4);
    bmc.release_all();
    let _first = first.await??;
    let _second = second.await??;
    let _third = third.await??;
    let _fourth = fourth.await??;
    assert_eq!(bmc.request_count()?, 4);
    Ok(())
}

#[tokio::test]
async fn coalescing_removes_inflight_entry_after_completion() -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "first"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let first_scraper = scraper.clone();
    let first = tokio::spawn(async move {
        first_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    bmc.release_all();
    let _first = first.await??;
    assert_eq!(bmc.request_count()?, 1);

    let _second = scraper
        .resources::<TestResource>()
        .refresh(id("/redfish/v1/Test/1"))
        .await?;
    assert_eq!(bmc.request_count()?, 2);
    Ok(())
}

#[tokio::test]
async fn coalescing_removes_inflight_entry_when_owner_is_cancelled() -> Result<(), Box<dyn StdError>>
{
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "after-cancel"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(4)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;

    let owner_scraper = scraper.clone();
    let owner = tokio::spawn(async move {
        owner_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    owner.abort();
    assert!(owner.await.is_err());
    yield_now().await;

    bmc.release_all();
    let snapshot = scraper
        .resources::<TestResource>()
        .refresh(id("/redfish/v1/Test/1"))
        .await?;

    assert_eq!(snapshot.value.name, "after-cancel");
    assert_eq!(bmc.request_count()?, 2);
    Ok(())
}

#[tokio::test]
async fn scheduler_stats_are_cleaned_when_admitted_request_is_cancelled(
) -> Result<(), Box<dyn StdError>> {
    let bmc = BlockingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/1", "cancelled"))?;
    let scraper = Scraper::builder(bmc.clone())
        .capacity(
            BmcCapacity::fixed()
                .max_in_flight(1)
                .max_requests_per_second(u32::MAX),
        )
        .build()
        .await?;
    let mut events = scraper.subscribe_events();

    let request_scraper = scraper.clone();
    let request = tokio::spawn(async move {
        request_scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/1"))
            .await
    });
    bmc.wait_for_in_flight(1).await?;
    request.abort();
    assert!(request.await.is_err());
    yield_now().await;

    let stats = drain_scheduler_stats(&mut events)?;
    assert!(stats.iter().any(|state| state.in_flight == 1));
    assert!(stats.iter().any(|state| state.in_flight == 0));
    Ok(())
}
