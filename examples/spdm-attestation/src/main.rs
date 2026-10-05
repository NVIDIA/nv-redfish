// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::error::Error as StdError;
use std::fs::File;
use std::io::Error as IoError;
use std::io::Read as _;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use nv_redfish::bmc_http::reqwest::Client;
use nv_redfish::bmc_http::reqwest::ClientParams;
use nv_redfish::bmc_http::BmcCredentials;
use nv_redfish::bmc_http::CacheSettings;
use nv_redfish::bmc_http::HttpBmc;
use nv_redfish::component_integrity::SpdmGetSignedMeasurementsResponse;
use nv_redfish::core::BmcError as _;
use nv_redfish::core::BmcErrorClass;
use nv_redfish::Error;
use nv_redfish::ServiceRoot;
use url::Url;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_POLLS: u32 = 60;

#[derive(Debug, Parser)]
#[command(about = "Collect SPDM signed measurements from a live BMC")]
struct Args {
    /// BMC base URL, for example https://10.0.0.1
    #[arg(long)]
    bmc: Url,

    #[arg(long)]
    username: String,

    #[arg(long)]
    password: String,

    /// ComponentIntegrity member to attest. Omit to list the members.
    #[arg(long)]
    component: Option<String>,

    /// Accept invalid TLS certificates.
    #[arg(long, default_value_t = false)]
    insecure: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();

    let client = Client::with_params(ClientParams::new().accept_invalid_certs(args.insecure))?;
    let bmc = Arc::new(HttpBmc::new(
        client,
        args.bmc,
        BmcCredentials::new(args.username, args.password),
        CacheSettings::with_capacity(0),
    ));
    let root = ServiceRoot::new(bmc.clone()).await?;
    let collection = root
        .component_integrity()
        .await?
        .ok_or_else(|| IoError::other("ComponentIntegrity is not advertised"))?;
    let components = collection.members().await?;

    println!("Discovered ComponentIntegrity members:");
    for component in &components {
        let raw = component.raw();
        println!(
            "  id={} enabled={:?} type={:?} version={} action={}",
            raw.id,
            raw.component_integrity_enabled,
            raw.component_integrity_type,
            raw.component_integrity_type_version,
            component.spdm_get_signed_measurements_target().is_some()
        );
    }

    let Some(requested_component) = args.component else {
        println!("Pass --component <id> to request signed measurements.");
        return Ok(());
    };
    let component = components
        .iter()
        .find(|component| component.raw().id == requested_component.as_str())
        .ok_or_else(|| {
            IoError::other(format!(
                "requested component {requested_component:?} was not found"
            ))
        })?;

    println!("Selected component: {}", component.raw().id);
    if let Some(certificate) = component.component_certificate().await? {
        let certificate = certificate.raw();
        println!(
            "Responder certificate: id={} type={:?} slot={:?}",
            certificate.id,
            certificate.certificate_type,
            certificate
                .spdm
                .as_ref()
                .and_then(|spdm| spdm.slot_id.as_ref())
                .and_then(Option::as_ref)
        );
    } else {
        println!("Responder certificate: not advertised");
    }

    let nonce = generate_nonce()?;
    println!("Requesting signed measurements with nonce {nonce}");
    let mut request = component
        .spdm_get_signed_measurements(Some(nonce), None, None)
        .await?;
    match request.pending_task() {
        Some(task) => println!("Action accepted; polling {}", task.location.0),
        None => println!("Action returned its result immediately"),
    }

    for poll in 1..=MAX_POLLS {
        match request.poll_result(bmc.as_ref()).await {
            Ok(None) => {
                let retry_after = request.retry_after();
                let wait = retry_after.unwrap_or(POLL_INTERVAL);
                let location = request
                    .pending_task()
                    .map_or_else(|| "<none>".to_string(), |task| task.location.0.to_string());
                println!(
                    "Poll {poll}: pending at {location} (Retry-After {}); next poll in {wait:?}",
                    retry_after.map_or_else(|| "absent".to_string(), |d| format!("{d:?}"))
                );
                tokio::time::sleep(wait).await;
            }
            Ok(Some(evidence)) => {
                println!("Poll {poll}: complete");
                print_evidence(&evidence);
                println!(
                    "Verify the signature against the request nonce before trusting this evidence."
                );
                return Ok(());
            }
            Err(Error::TaskFailed { state, messages }) => {
                println!("Poll {poll}: Task ended in state {state:?}");
                for message in &messages {
                    println!(
                        "  {}: {}",
                        message.message_id,
                        message.message.as_deref().unwrap_or("<no message>")
                    );
                }
                return Err(IoError::other("SPDM measurement Task failed").into());
            }
            // A failed request leaves the handle on the same step, so the
            // next poll retries it. Client errors will not succeed on retry.
            Err(Error::Bmc(error))
                if !matches!(
                    error.error_class(),
                    BmcErrorClass::HttpResponse { status } if status < 500
                ) =>
            {
                println!("Poll {poll}: BMC request failed, retrying: {error}");
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            Err(error) => return Err(error.into()),
        }
    }

    Err(IoError::other(format!(
        "measurements were not ready after {MAX_POLLS} polls"
    ))
    .into())
}

fn print_evidence(evidence: &SpdmGetSignedMeasurementsResponse) {
    println!("SPDM evidence:");
    println!("  version: {}", evidence.version);
    println!("  hashing algorithm: {}", evidence.hashing_algorithm);
    println!("  signing algorithm: {}", evidence.signing_algorithm);
    println!(
        "  signed measurements length: {}",
        evidence.signed_measurements.len()
    );
}

fn generate_nonce() -> Result<String, IoError> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut nonce = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        nonce.push(char::from(HEX[usize::from(byte >> 4)]));
        nonce.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(nonce)
}
