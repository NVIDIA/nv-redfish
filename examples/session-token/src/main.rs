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

use clap::Parser;
use nv_redfish::bmc_http::reqwest::Client;
use nv_redfish::bmc_http::reqwest::ClientParams;
use nv_redfish::bmc_http::BmcCredentials;
use nv_redfish::bmc_http::CacheSettings;
use nv_redfish::bmc_http::HttpBmc;
use nv_redfish::session_service::SessionCreate;
use nv_redfish::ServiceRoot;
use std::error::Error as StdError;
use std::sync::Arc;
use url::Url;

#[derive(Debug, Parser)]
#[command()]
struct Args {
    #[arg(long)]
    bmc: Url,

    #[arg(long)]
    username: String,

    #[arg(long)]
    password: String,

    #[arg(long, default_value_t = false)]
    insecure: bool,

    #[arg(long, default_value_t = false)]
    print_token: bool,

    #[arg(long, default_value_t = true)]
    delete_session: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();
    let client = Client::with_params(ClientParams::new().accept_invalid_certs(args.insecure))?;
    let bmc = Arc::new(HttpBmc::new(
        client,
        args.bmc,
        BmcCredentials::new(args.username.clone(), args.password.clone()),
        CacheSettings::default(),
    ));

    let root = ServiceRoot::new(Arc::clone(&bmc)).await?;
    let session_service = root
        .session_service()
        .await?
        .ok_or("BMC did not expose SessionService")?;
    let sessions = session_service
        .sessions()
        .await?
        .ok_or("SessionService did not expose Sessions collection")?;
    let session = sessions
        .create_session(&SessionCreate::builder(args.username, args.password).build())
        .await?;
    let token = session
        .auth_token()
        .ok_or("Session creation response did not include X-Auth-Token")?
        .to_string();

    println!("Session data: {:#?}", session.raw());

    if args.print_token {
        println!("Token: {token}");
    } else {
        println!("Token acquired from X-Auth-Token header.");
    }

    bmc.set_credentials(BmcCredentials::token(token));
    let token_root = ServiceRoot::new(Arc::clone(&bmc)).await?;
    println!(
        "Authenticated with session token. Vendor: {:?}",
        token_root.vendor()
    );

    if args.delete_session {
        let _ = session.delete().await?;
        println!("Deleted created session.");
    }

    // should fail
    token_root.chassis().await?;

    Ok(())
}
