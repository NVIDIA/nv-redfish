use super::*;

#[test]
fn store_records_relation_between_sensor_and_drive() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Sensor1", "temperature"))?;
    let scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;
    let relation = relation("/redfish/v1/Test/Sensor1", "/redfish/v1/Drives/1");

    let _snapshot = tokio_test::block_on(
        scraper
            .resources::<TestResource>()
            .refresh(id("/redfish/v1/Test/Sensor1")),
    )?;
    scraper.record_relation(relation)?;

    assert!(scraper
        .inner()
        .store
        .has_relation_to_type::<TestResource, OtherResource>(&id("/redfish/v1/Test/Sensor1")));
    Ok(())
}

#[test]
fn related_to_predicate_filters_by_relation_index() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Sensor1", "temperature"))?;
    bmc.insert(&resource("/redfish/v1/Test/Sensor2", "temperature"))?;
    let relation = relation("/redfish/v1/Test/Sensor1", "/redfish/v1/Drives/1");
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::manual::<TestResource, _>(
                TestDiscoverer::new(vec![
                    id("/redfish/v1/Test/Sensor1"),
                    id("/redfish/v1/Test/Sensor2"),
                ])
                .with_relations(vec![relation]),
            ))
            .build(),
    )?;

    let resources = tokio_test::block_on(
        scraper
            .query::<TestResource>()
            .where_(resource_predicate::related_to::<OtherResource>())
            .list(),
    )?;

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, id("/redfish/v1/Test/Sensor1"));
    Ok(())
}

#[test]
fn relation_discovery_hint_reaches_discoverer() -> Result<(), Box<dyn StdError>> {
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
            .where_(resource_predicate::related_to::<OtherResource>())
            .list(),
    )?;

    assert!(resources.is_empty());
    let hints = recorded.hints()?;
    assert_eq!(hints.len(), 1);
    assert_eq!(
        hints[0].relation_target_types(),
        &[TypeId::of::<OtherResource>()]
    );
    Ok(())
}

#[tokio::test]
async fn resource_update_re_evaluates_relation_based_query() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Sensor1", "temperature"))?;
    let scraper = Scraper::builder(bmc)
        .discover(Discovery::manual::<TestResource, _>(TestDiscoverer::new(
            vec![id("/redfish/v1/Test/Sensor1")],
        )))
        .build()
        .await?;
    let mut subscription = scraper
        .query::<TestResource>()
        .where_(resource_predicate::related_to::<OtherResource>())
        .subscribe()
        .await?;

    scraper.record_relation(relation("/redfish/v1/Test/Sensor1", "/redfish/v1/Drives/1"))?;
    match subscription.recv().await? {
        TypedResourceEvent::Added(snapshot) => {
            assert_eq!(snapshot.id, id("/redfish/v1/Test/Sensor1"));
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

#[tokio::test]
async fn relation_removal_emits_removed_for_query() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert(&resource("/redfish/v1/Test/Sensor1", "temperature"))?;
    let relation = relation("/redfish/v1/Test/Sensor1", "/redfish/v1/Drives/1");
    let scraper = Scraper::builder(bmc)
        .discover(Discovery::manual::<TestResource, _>(
            TestDiscoverer::new(vec![id("/redfish/v1/Test/Sensor1")])
                .with_relations(vec![relation.clone()]),
        ))
        .build()
        .await?;
    let mut subscription = scraper
        .query::<TestResource>()
        .where_(resource_predicate::related_to::<OtherResource>())
        .subscribe()
        .await?;
    match subscription.recv().await? {
        TypedResourceEvent::Added(snapshot) => {
            assert_eq!(snapshot.id, id("/redfish/v1/Test/Sensor1"));
        }
        TypedResourceEvent::Updated { .. }
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected added event")));
        }
    }

    scraper.remove_relation(&relation)?;
    match subscription.recv().await? {
        TypedResourceEvent::Removed(id) => {
            assert_eq!(id, super::id("/redfish/v1/Test/Sensor1"));
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
