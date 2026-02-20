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

#![recursion_limit = "256"]

use clap::Parser;
use futures_util::TryStreamExt;
use nv_redfish::bmc_http::reqwest::{Client, ClientParams};
use nv_redfish::bmc_http::{BmcCredentials, CacheSettings, HttpBmc};
use nv_redfish::event_service::EventStreamPayload;
use nv_redfish::ServiceRoot;
use std::error::Error as StdError;
use std::sync::Arc;
use url::Url;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg(long)]
    bmc: Url,

    #[arg(long, requires = "password")]
    username: Option<String>,

    #[arg(long, requires = "username")]
    password: Option<String>,

    #[arg(long, default_value_t = false)]
    insecure: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();

    let client = Client::with_params(
        ClientParams::new()
            .accept_invalid_certs(args.insecure)
            .no_timeout(),
    )?;
    let credentials = BmcCredentials::new(
        args.username.unwrap_or_default(),
        args.password.unwrap_or_default(),
    );

    let bmc = Arc::new(HttpBmc::new(
        client,
        args.bmc,
        credentials,
        CacheSettings::default(),
    ));

    let root = ServiceRoot::new(Arc::clone(&bmc)).await?;

    let event_service = root.event_service().await?;
    let mut stream = event_service.events().await?;

    println!("Connected to EventService stream. Waiting for events...");

    let mut event_index = 0_u64;
    while let Some(payload) = stream.try_next().await? {
        event_index += 1;
        match payload {
            EventStreamPayload::Event(event) => {
                println!("[{event_index}] Event: {event:?}");
            }
            EventStreamPayload::MetricReport(report) => {
                println!("[{event_index}] MetricReport: {report:?}");
            }
        }
    }

    Ok(())
}
