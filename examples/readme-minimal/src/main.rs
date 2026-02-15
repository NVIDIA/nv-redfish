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

// Recursion limit is need to be increased because Redfish has deep
// tree of types reference to each other.
#![recursion_limit = "256"]

use nv_redfish::ServiceRoot;
use nv_redfish_bmc_http::reqwest::Client;
use nv_redfish_bmc_http::{BmcCredentials, CacheSettings, HttpBmc};
use std::sync::Arc;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = Client::new()?;
    let bmc = Arc::new(HttpBmc::new(
        http_client,
        Url::parse("https://example.com")?,
        BmcCredentials::new("admin".into(), "password".into()),
        CacheSettings::default(),
    ));

    let root = ServiceRoot::new(Arc::clone(&bmc)).await?;
    println!("BMC vendor: {:?}", root.vendor());
    Ok(())
}
