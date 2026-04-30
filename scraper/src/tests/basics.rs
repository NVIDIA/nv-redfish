use super::*;

#[test]
fn builder_creates_scraper() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();

    let _scraper = tokio_test::block_on(Scraper::builder(bmc).build())?;

    assert_eq!(request_counter.request_count()?, 0);
    Ok(())
}

#[test]
fn scraper_is_cloneable() -> Result<(), Box<dyn StdError>> {
    let scraper = tokio_test::block_on(Scraper::builder(RecordingBmc::default()).build())?;
    let cloned = scraper.clone();

    let _resources = scraper.resources::<Sensor>();
    let _query = cloned.query::<Sensor>();

    Ok(())
}

#[test]
fn subscribe_events_returns_stream() -> Result<(), Box<dyn StdError>> {
    let scraper = tokio_test::block_on(Scraper::builder(RecordingBmc::default()).build())?;
    let events = scraper.subscribe_events();

    assert_eq!(events.len(), 0);
    Ok(())
}

#[test]
fn discovery_registration_does_not_call_bmc() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();

    let _scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .capacity(BmcCapacity::adaptive())
            .discover(Discovery::standard())
            .build(),
    )?;

    assert_eq!(request_counter.request_count()?, 0);
    Ok(())
}
