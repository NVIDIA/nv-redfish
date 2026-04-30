use super::*;

#[test]
fn standard_discovery_finds_chassis_sensors() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    insert_standard_sensor_root(
        &bmc,
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            None,
        ),
        Some(sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &[
                "/redfish/v1/Chassis/1/Sensors/Inlet",
                "/redfish/v1/Chassis/1/Sensors/Outlet",
            ],
        )),
        None,
        &[
            ("/redfish/v1/Chassis/1/Sensors/Inlet", "Inlet"),
            ("/redfish/v1/Chassis/1/Sensors/Outlet", "Outlet"),
        ],
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(scraper.query::<RedfishSensor>().list())?;
    let ids = sensors
        .iter()
        .map(|sensor| sensor.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            id("/redfish/v1/Chassis/1/Sensors/Inlet"),
            id("/redfish/v1/Chassis/1/Sensors/Outlet"),
        ]
    );
    Ok(())
}

#[test]
fn standard_discovery_finds_environment_metric_sensor_uris() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    insert_standard_sensor_root(
        &bmc,
        chassis(
            "/redfish/v1/Chassis/1",
            None,
            Some("/redfish/v1/Chassis/1/EnvironmentMetrics"),
        ),
        None,
        Some(environment_metrics(
            "/redfish/v1/Chassis/1/EnvironmentMetrics",
            "/redfish/v1/Chassis/1/Sensors/Ambient",
        )),
        &[("/redfish/v1/Chassis/1/Sensors/Ambient", "Ambient")],
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(scraper.query::<RedfishSensor>().list())?;

    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].id, id("/redfish/v1/Chassis/1/Sensors/Ambient"));
    Ok(())
}

#[test]
fn standard_discovery_deduplicates_sensor_ids() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();
    insert_standard_sensor_root(
        &bmc,
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            Some("/redfish/v1/Chassis/1/EnvironmentMetrics"),
        ),
        Some(sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &["/redfish/v1/Chassis/1/Sensors/Shared"],
        )),
        Some(environment_metrics(
            "/redfish/v1/Chassis/1/EnvironmentMetrics",
            "/redfish/v1/Chassis/1/Sensors/Shared",
        )),
        &[("/redfish/v1/Chassis/1/Sensors/Shared", "Shared")],
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(scraper.query::<RedfishSensor>().list())?;
    let sensor_requests = request_counter
        .requested_ids()?
        .into_iter()
        .filter(|requested| requested == &id("/redfish/v1/Chassis/1/Sensors/Shared"))
        .count();

    assert_eq!(sensors.len(), 1);
    assert_eq!(sensor_requests, 1);
    Ok(())
}

#[test]
fn standard_discovery_is_incremental() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    insert_standard_sensor_root(
        &bmc,
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            None,
        ),
        Some(sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &["/redfish/v1/Chassis/1/Sensors/Inlet"],
        )),
        None,
        &[("/redfish/v1/Chassis/1/Sensors/Inlet", "Inlet")],
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(scraper.query::<RedfishSensor>().list())?;
    let discovery_records = scraper
        .inner()
        .scheduler
        .records()?
        .into_iter()
        .filter(|record| record.lane == Lane::Discovery)
        .collect::<Vec<_>>();

    assert_eq!(sensors.len(), 1);
    assert!(discovery_records.len() >= 4);
    assert!(discovery_records
        .iter()
        .all(|record| record.operation == Operation::Get));
    Ok(())
}

#[test]
fn standard_discovery_does_not_assume_global_sensor_path() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();
    insert_standard_sensor_root(
        &bmc,
        chassis(
            "/redfish/v1/Chassis/1",
            Some("/redfish/v1/Chassis/1/Sensors"),
            None,
        ),
        Some(sensor_collection(
            "/redfish/v1/Chassis/1/Sensors",
            &["/redfish/v1/Chassis/1/Sensors/Inlet"],
        )),
        None,
        &[("/redfish/v1/Chassis/1/Sensors/Inlet", "Inlet")],
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let sensors = tokio_test::block_on(scraper.query::<RedfishSensor>().list())?;

    assert_eq!(sensors.len(), 1);
    assert!(!request_counter
        .requested_ids()?
        .contains(&id("/redfish/v1/Sensors")));
    Ok(())
}

#[test]
fn standard_discovery_finds_firmware_inventory() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(None, None, None, Some("/redfish/v1/UpdateService")),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService"),
        update_service(
            "/redfish/v1/UpdateService",
            Some("/redfish/v1/UpdateService/FirmwareInventory"),
            None,
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory"),
        software_inventory_collection(
            "/redfish/v1/UpdateService/FirmwareInventory",
            &[
                "/redfish/v1/UpdateService/FirmwareInventory/BMC",
                "/redfish/v1/UpdateService/FirmwareInventory/BIOS",
            ],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory/BMC"),
        software_inventory(
            "/redfish/v1/UpdateService/FirmwareInventory/BMC",
            "BMC",
            "1.0.0",
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory/BIOS"),
        software_inventory(
            "/redfish/v1/UpdateService/FirmwareInventory/BIOS",
            "BIOS",
            "2.0.0",
        ),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let firmware = tokio_test::block_on(scraper.query::<SoftwareInventory>().list())?;
    let ids = firmware
        .iter()
        .map(|snapshot| snapshot.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            id("/redfish/v1/UpdateService/FirmwareInventory/BIOS"),
            id("/redfish/v1/UpdateService/FirmwareInventory/BMC"),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn firmware_query_emits_added_and_updated() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(None, None, None, Some("/redfish/v1/UpdateService")),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService"),
        update_service(
            "/redfish/v1/UpdateService",
            Some("/redfish/v1/UpdateService/FirmwareInventory"),
            None,
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory"),
        software_inventory_collection(
            "/redfish/v1/UpdateService/FirmwareInventory",
            &["/redfish/v1/UpdateService/FirmwareInventory/BMC"],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory/BMC"),
        software_inventory(
            "/redfish/v1/UpdateService/FirmwareInventory/BMC",
            "BMC",
            "1.0.0",
        ),
    )?;
    let scraper = Scraper::builder(bmc.clone())
        .discover(Discovery::standard())
        .build()
        .await?;
    let mut firmware = scraper.query::<SoftwareInventory>().subscribe().await?;

    match firmware.recv().await? {
        TypedResourceEvent::Added(snapshot) => {
            assert_eq!(
                snapshot.id,
                id("/redfish/v1/UpdateService/FirmwareInventory/BMC")
            );
        }
        TypedResourceEvent::Updated { .. }
        | TypedResourceEvent::Removed(_)
        | TypedResourceEvent::FreshnessMissed { .. }
        | TypedResourceEvent::Error { .. } => {
            return Err(TestFailure::boxed(String::from("expected added event")));
        }
    }
    bmc.insert_value(
        id("/redfish/v1/UpdateService/FirmwareInventory/BMC"),
        software_inventory(
            "/redfish/v1/UpdateService/FirmwareInventory/BMC",
            "BMC",
            "1.1.0",
        ),
    )?;
    let _snapshot = scraper
        .resources::<SoftwareInventory>()
        .refresh(id("/redfish/v1/UpdateService/FirmwareInventory/BMC"))
        .await?;

    match firmware.recv().await? {
        TypedResourceEvent::Updated { new, .. } => {
            assert_eq!(
                new.id,
                id("/redfish/v1/UpdateService/FirmwareInventory/BMC")
            );
            assert_eq!(
                new.value.version.as_ref().and_then(Option::as_deref),
                Some("1.1.0")
            );
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
fn standard_discovery_finds_log_services_from_chassis() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(Some("/redfish/v1/Chassis"), None, None, None),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis"),
        chassis_collection("/redfish/v1/Chassis", &["/redfish/v1/Chassis/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1"),
        chassis_with_log_services("/redfish/v1/Chassis/1", "/redfish/v1/Chassis/1/LogServices"),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/LogServices"),
        log_service_collection(
            "/redfish/v1/Chassis/1/LogServices",
            &["/redfish/v1/Chassis/1/LogServices/EventLog"],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1/LogServices/EventLog"),
        log_service("/redfish/v1/Chassis/1/LogServices/EventLog", "Event Log"),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let logs = tokio_test::block_on(scraper.query::<LogService>().list())?;

    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].id, id("/redfish/v1/Chassis/1/LogServices/EventLog"));
    Ok(())
}

#[test]
fn standard_discovery_finds_log_services_from_systems() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(None, Some("/redfish/v1/Systems"), None, None),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems"),
        system_collection("/redfish/v1/Systems", &["/redfish/v1/Systems/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems/1"),
        computer_system(
            "/redfish/v1/Systems/1",
            Some("/redfish/v1/Systems/1/LogServices"),
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems/1/LogServices"),
        log_service_collection(
            "/redfish/v1/Systems/1/LogServices",
            &["/redfish/v1/Systems/1/LogServices/SystemLog"],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems/1/LogServices/SystemLog"),
        log_service("/redfish/v1/Systems/1/LogServices/SystemLog", "System Log"),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let logs = tokio_test::block_on(scraper.query::<LogService>().list())?;

    assert_eq!(logs.len(), 1);
    assert_eq!(
        logs[0].id,
        id("/redfish/v1/Systems/1/LogServices/SystemLog")
    );
    Ok(())
}

#[test]
fn standard_discovery_finds_log_services_from_managers() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(None, None, Some("/redfish/v1/Managers"), None),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Managers"),
        manager_collection("/redfish/v1/Managers", &["/redfish/v1/Managers/BMC"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Managers/BMC"),
        manager(
            "/redfish/v1/Managers/BMC",
            Some("/redfish/v1/Managers/BMC/LogServices"),
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Managers/BMC/LogServices"),
        log_service_collection(
            "/redfish/v1/Managers/BMC/LogServices",
            &["/redfish/v1/Managers/BMC/LogServices/ManagerLog"],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Managers/BMC/LogServices/ManagerLog"),
        log_service(
            "/redfish/v1/Managers/BMC/LogServices/ManagerLog",
            "Manager Log",
        ),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let logs = tokio_test::block_on(scraper.query::<LogService>().list())?;

    assert_eq!(logs.len(), 1);
    assert_eq!(
        logs[0].id,
        id("/redfish/v1/Managers/BMC/LogServices/ManagerLog")
    );
    Ok(())
}

#[test]
fn log_service_discovery_deduplicates_services() -> Result<(), Box<dyn StdError>> {
    let bmc = RecordingBmc::default();
    let request_counter = bmc.clone();
    bmc.insert_value(
        id("/redfish/v1"),
        service_root_with_links(
            Some("/redfish/v1/Chassis"),
            Some("/redfish/v1/Systems"),
            None,
            None,
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis"),
        chassis_collection("/redfish/v1/Chassis", &["/redfish/v1/Chassis/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Chassis/1"),
        chassis_with_log_services("/redfish/v1/Chassis/1", "/redfish/v1/SharedLogServices"),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems"),
        system_collection("/redfish/v1/Systems", &["/redfish/v1/Systems/1"]),
    )?;
    bmc.insert_value(
        id("/redfish/v1/Systems/1"),
        computer_system(
            "/redfish/v1/Systems/1",
            Some("/redfish/v1/SharedLogServices"),
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/SharedLogServices"),
        log_service_collection(
            "/redfish/v1/SharedLogServices",
            &["/redfish/v1/SharedLogServices/EventLog"],
        ),
    )?;
    bmc.insert_value(
        id("/redfish/v1/SharedLogServices/EventLog"),
        log_service("/redfish/v1/SharedLogServices/EventLog", "Shared Event Log"),
    )?;
    let scraper = tokio_test::block_on(
        Scraper::builder(bmc)
            .discover(Discovery::standard())
            .build(),
    )?;

    let logs = tokio_test::block_on(scraper.query::<LogService>().list())?;
    let service_requests = request_counter
        .requested_ids()?
        .into_iter()
        .filter(|requested| requested == &id("/redfish/v1/SharedLogServices/EventLog"))
        .count();

    assert_eq!(logs.len(), 1);
    assert_eq!(service_requests, 1);
    Ok(())
}
