// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use nv_redfish::bmc_http::reqwest::Client;
use nv_redfish::bmc_http::reqwest::ClientParams;
use nv_redfish::bmc_http::BmcCredentials;
use nv_redfish::bmc_http::CacheSettings;
use nv_redfish::bmc_http::HttpBmc;
use nv_redfish::schema::chassis::Chassis;
use nv_redfish::schema::log_service::LogService;
use nv_redfish::schema::sensor::Sensor;
use nv_redfish::schema::software_inventory::SoftwareInventory;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::ODataId;
use nv_redfish_scraper::BmcCapacity;
use nv_redfish_scraper::Discoverer;
use nv_redfish_scraper::Discovery;
use nv_redfish_scraper::DiscoveryBatch;
use nv_redfish_scraper::DiscoveryContext;
use nv_redfish_scraper::DiscoveryHint;
use nv_redfish_scraper::RawResource;
use nv_redfish_scraper::ResourceSnapshot;
use nv_redfish_scraper::Scraper;
use nv_redfish_scraper::TypedResourceEvent;
use serde::de::DeserializeOwned;
use serde_json::json;
use serde_json::Value;
use url::Url;

type RealBmc = HttpBmc<Client>;
type Result<T> = std::result::Result<T, Box<dyn StdError>>;

#[derive(Parser, Debug)]
#[command(about = "Example Redfish scraper CLI for real BMCs")]
struct Args {
    #[arg(long)]
    bmc: Url,

    #[arg(long, requires = "password")]
    username: Option<String>,

    #[arg(long, requires = "username")]
    password: Option<String>,

    #[arg(long, default_value_t = false)]
    insecure: bool,

    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,

    #[arg(long, default_value_t = 16)]
    max_in_flight: usize,

    #[arg(long)]
    initial_in_flight: Option<usize>,

    #[arg(long, default_value_t = 200)]
    max_requests_per_second: u32,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Fetch any explicit Redfish URI as raw JSON.
    Get(GetArgs),
    /// Discover and refresh a typed resource set once.
    List(ListArgs),
    /// Subscribe to a typed resource set and poll it in the background.
    Stream(StreamArgs),
}

#[derive(Debug, Parser)]
struct GetArgs {
    path: String,

    #[arg(long, value_enum, default_value_t = Output::Json)]
    output: Output,
}

#[derive(Debug, Parser)]
struct ListArgs {
    #[arg(value_enum)]
    kind: ResourceKind,

    #[arg(long, value_enum, default_value_t = Output::Json)]
    output: Output,
}

#[derive(Debug, Parser)]
struct StreamArgs {
    #[arg(value_enum)]
    kind: ResourceKind,

    #[arg(long, default_value_t = 5)]
    freshness_secs: u64,

    #[arg(long, default_value_t = 60)]
    discovery_freshness_secs: u64,

    #[arg(long)]
    count: Option<usize>,

    #[arg(long, value_enum, default_value_t = Output::Json)]
    output: Output,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResourceKind {
    Sensors,
    Chassis,
    Firmware,
    LogServices,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Output {
    Json,
    Table,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let scraper = build_scraper(&args).await?;

    match args.command {
        Command::Get(command) => run_get(&scraper, command).await?,
        Command::List(command) => run_list(&scraper, command).await?,
        Command::Stream(command) => run_stream(&scraper, command).await?,
    }

    Ok(())
}

async fn build_scraper(args: &Args) -> Result<Scraper<RealBmc>> {
    let client = Client::with_params(
        ClientParams::new()
            .accept_invalid_certs(args.insecure)
            .timeout(Duration::from_secs(args.timeout_secs)),
    )?;
    let credentials = BmcCredentials::new(
        args.username.clone().unwrap_or_default(),
        args.password.clone().unwrap_or_default(),
    );
    let bmc = HttpBmc::new(
        client,
        args.bmc.clone(),
        credentials,
        CacheSettings::default(),
    );
    let capacity = BmcCapacity::adaptive()
        .initial_in_flight(args.initial_in_flight.unwrap_or(args.max_in_flight))
        .max_in_flight(args.max_in_flight)
        .max_requests_per_second(args.max_requests_per_second);

    Ok(Scraper::builder(bmc)
        .capacity(capacity)
        .discover(Discovery::standard())
        .discover(Discovery::manual::<Chassis, _>(
            RootCollectionDiscoverer::new("Chassis"),
        ))
        .build()
        .await?)
}

async fn run_get(scraper: &Scraper<RealBmc>, command: GetArgs) -> Result<()> {
    let snapshot = scraper
        .raw_resources()
        .refresh(ODataId::from(command.path))
        .await?;
    print_raw_snapshot(&snapshot, command.output)
}

async fn run_list(scraper: &Scraper<RealBmc>, command: ListArgs) -> Result<()> {
    match command.kind {
        ResourceKind::Sensors => list_typed::<Sensor>(scraper, command.output).await,
        ResourceKind::Chassis => list_typed::<Chassis>(scraper, command.output).await,
        ResourceKind::Firmware => list_typed::<SoftwareInventory>(scraper, command.output).await,
        ResourceKind::LogServices => list_typed::<LogService>(scraper, command.output).await,
    }
}

async fn run_stream(scraper: &Scraper<RealBmc>, command: StreamArgs) -> Result<()> {
    match command.kind {
        ResourceKind::Sensors => stream_typed::<Sensor>(scraper, &command).await,
        ResourceKind::Chassis => stream_typed::<Chassis>(scraper, &command).await,
        ResourceKind::Firmware => stream_typed::<SoftwareInventory>(scraper, &command).await,
        ResourceKind::LogServices => stream_typed::<LogService>(scraper, &command).await,
    }
}

async fn list_typed<T>(scraper: &Scraper<RealBmc>, output: Output) -> Result<()>
where
    T: CliResource,
{
    let snapshots = scraper.query::<T>().list().await?;
    match output {
        Output::Json => print_pretty(&json!(snapshots
            .iter()
            .map(snapshot_json)
            .collect::<std::result::Result<Vec<_>, _>>()?)),
        Output::Table => {
            print_table_header();
            for snapshot in snapshots {
                print_table_row(&snapshot_json(&snapshot)?);
            }
            Ok(())
        }
    }
}

async fn stream_typed<T>(scraper: &Scraper<RealBmc>, command: &StreamArgs) -> Result<()>
where
    T: CliResource,
{
    let mut subscription = scraper
        .query::<T>()
        .freshness(Duration::from_secs(command.freshness_secs))
        .discovery_freshness(Duration::from_secs(command.discovery_freshness_secs))
        .subscribe()
        .await?;
    let mut seen = 0_usize;

    if matches!(command.output, Output::Table) {
        print_event_table_header();
    }

    loop {
        if command.count.is_some_and(|count| seen >= count) {
            return Ok(());
        }
        tokio::select! {
            event = subscription.recv() => {
                seen += 1;
                print_typed_event(&event?, command.output)?;
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                return Ok(());
            }
        }
    }
}

fn print_typed_event<T>(event: &TypedResourceEvent<T>, output: Output) -> Result<()>
where
    T: CliResource,
{
    let value = event_json(event)?;
    match output {
        Output::Json => println!("{}", serde_json::to_string(&value)?),
        Output::Table => print_event_table_row(&value),
    }
    Ok(())
}

fn event_json<T>(event: &TypedResourceEvent<T>) -> Result<Value>
where
    T: CliResource,
{
    Ok(match event {
        TypedResourceEvent::Added(snapshot) => {
            json!({"event": "added", "snapshot": snapshot_json(snapshot)?})
        }
        TypedResourceEvent::Updated { new, .. } => {
            json!({"event": "updated", "snapshot": snapshot_json(new)?})
        }
        TypedResourceEvent::Removed(id) => {
            json!({"event": "removed", "id": id.to_string()})
        }
        TypedResourceEvent::FreshnessMissed { id, age, desired } => json!({
            "event": "freshness-missed",
            "id": id.to_string(),
            "age_ms": age.as_millis(),
            "desired_ms": desired.as_millis(),
        }),
        TypedResourceEvent::Error { id, error } => json!({
            "event": "error",
            "id": id.to_string(),
            "error": error.to_string(),
        }),
    })
}

fn snapshot_json<T>(snapshot: &ResourceSnapshot<T>) -> Result<Value>
where
    T: CliResource,
{
    Ok(json!({
        "id": snapshot.id.to_string(),
        "etag": snapshot.etag.as_ref().map(ToString::to_string),
        "fetched_at_unix_ms": unix_millis(snapshot.fetched_at),
        "staleness": format!("{:?}", snapshot.staleness),
        "value": snapshot.value.summary(),
    }))
}

fn raw_snapshot_json(snapshot: &ResourceSnapshot<RawResource>) -> Value {
    json!({
        "id": snapshot.id.to_string(),
        "etag": snapshot.etag.as_ref().map(ToString::to_string),
        "fetched_at_unix_ms": unix_millis(snapshot.fetched_at),
        "staleness": format!("{:?}", snapshot.staleness),
        "value": snapshot.value.value(),
    })
}

fn print_raw_snapshot(snapshot: &ResourceSnapshot<RawResource>, output: Output) -> Result<()> {
    let value = raw_snapshot_json(snapshot);
    match output {
        Output::Json => print_pretty(&value),
        Output::Table => {
            print_table_header();
            print_table_row(&value);
            Ok(())
        }
    }
}

fn print_pretty(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_table_header() {
    println!(
        "{:<72}  {:<32}  {:<20}  {:<20}",
        "ID", "NAME", "TYPE", "STATE"
    );
}

fn print_table_row(value: &Value) {
    let id = value.get("id").and_then(Value::as_str).unwrap_or("-");
    let body = value.get("value").unwrap_or(&Value::Null);
    let name = body.get("Name").and_then(Value::as_str).unwrap_or("-");
    let resource_type = body
        .get("@odata.type")
        .and_then(Value::as_str)
        .or_else(|| body.get("ReadingType").and_then(Value::as_str))
        .unwrap_or("-");
    let staleness = value
        .get("staleness")
        .and_then(Value::as_str)
        .unwrap_or("-");
    println!("{id:<72}  {name:<32}  {resource_type:<20}  {staleness:<20}");
}

fn print_event_table_header() {
    println!(
        "{:<18}  {:<72}  {:<32}  {:<20}",
        "EVENT", "ID", "NAME", "STATE"
    );
}

fn print_event_table_row(value: &Value) {
    let event = value.get("event").and_then(Value::as_str).unwrap_or("-");
    let snapshot = value.get("snapshot").unwrap_or(value);
    let id = snapshot
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| value.get("id").and_then(Value::as_str))
        .unwrap_or("-");
    let body = snapshot.get("value").unwrap_or(&Value::Null);
    let name = body.get("Name").and_then(Value::as_str).unwrap_or("-");
    let state = snapshot
        .get("staleness")
        .and_then(Value::as_str)
        .or_else(|| value.get("error").and_then(Value::as_str))
        .unwrap_or("-");
    println!("{event:<18}  {id:<72}  {name:<32}  {state:<20}");
}

fn unix_millis(time: SystemTime) -> Option<u128> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

trait CliResource: EntityTypeRef + DeserializeOwned + Send + Sync + 'static {
    fn summary(&self) -> Value;
}

impl CliResource for Sensor {
    fn summary(&self) -> Value {
        json!({
            "@odata.type": "Sensor",
            "Name": &self.base.name,
            "ReadingType": self.reading_type.as_ref().and_then(Option::as_ref).map(|value| format!("{value:?}")),
            "PhysicalContext": self.physical_context.as_ref().and_then(Option::as_ref).map(|value| format!("{value:?}")),
            "Reading": self.reading.as_ref().and_then(Option::as_ref).map(ToString::to_string),
            "ReadingUnits": nested_string(&self.reading_units),
        })
    }
}

impl CliResource for Chassis {
    fn summary(&self) -> Value {
        json!({
            "@odata.type": "Chassis",
            "Name": &self.base.name,
            "ChassisType": format!("{:?}", self.chassis_type),
            "Manufacturer": nested_string(&self.manufacturer),
            "Model": nested_string(&self.model),
            "SerialNumber": nested_string(&self.serial_number),
            "PartNumber": nested_string(&self.part_number),
        })
    }
}

impl CliResource for SoftwareInventory {
    fn summary(&self) -> Value {
        json!({
            "@odata.type": "SoftwareInventory",
            "Name": &self.base.name,
            "Version": nested_string(&self.version),
            "SoftwareId": self.software_id.as_deref(),
            "Manufacturer": nested_string(&self.manufacturer),
        })
    }
}

impl CliResource for LogService {
    fn summary(&self) -> Value {
        json!({
            "@odata.type": "LogService",
            "Name": &self.base.name,
            "ServiceEnabled": self.service_enabled.as_ref().and_then(Option::as_ref),
            "MaxNumberOfRecords": self.max_number_of_records.as_ref(),
            "OverWritePolicy": self.over_write_policy.as_ref().map(|value| format!("{value:?}")),
            "LogEntryType": self.log_entry_type.as_ref().and_then(Option::as_ref).map(|value| format!("{value:?}")),
        })
    }
}

fn nested_string(value: &Option<Option<String>>) -> Option<&str> {
    value.as_ref().and_then(Option::as_deref)
}

#[derive(Clone, Debug)]
struct RootCollectionDiscoverer {
    property: &'static str,
}

impl RootCollectionDiscoverer {
    const fn new(property: &'static str) -> Self {
        Self { property }
    }
}

impl Discoverer<Chassis> for RootCollectionDiscoverer {
    fn discover<'a>(
        &'a self,
        cx: &'a mut DiscoveryContext<'a>,
        _hint: DiscoveryHint,
    ) -> Pin<
        Box<
            dyn Future<Output = std::result::Result<DiscoveryBatch, nv_redfish_scraper::Error>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let root = cx.fetch_raw(ODataId::service_root()).await?;
            let Some(collection_id) = raw_link(root.value(), self.property) else {
                return Ok(DiscoveryBatch::default());
            };
            let collection = cx.fetch_raw(collection_id).await?;
            Ok(DiscoveryBatch::candidates(raw_members(collection.value())))
        })
    }
}

fn raw_link(value: &Value, property: &str) -> Option<ODataId> {
    value
        .get(property)
        .and_then(|property| property.get("@odata.id"))
        .and_then(Value::as_str)
        .map(|id| ODataId::from(id.to_owned()))
}

fn raw_members(value: &Value) -> Vec<ODataId> {
    value
        .get("Members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| member.get("@odata.id").and_then(Value::as_str))
        .map(|id| ODataId::from(id.to_owned()))
        .collect()
}
