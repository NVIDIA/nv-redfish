// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use crate::BmcCredentials;
use crate::CacheableError;
use crate::HttpClient;
use futures_util::StreamExt;
use http::header;
use http::HeaderMap;
use nv_redfish_core::BoxTryStream;
use nv_redfish_core::Empty;
use nv_redfish_core::ODataETag;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;
use url::Url;

#[derive(Debug)]
pub enum BmcError {
    ReqwestError(reqwest::Error),
    JsonError(serde_path_to_error::Error<serde_json::Error>),
    InvalidResponse(reqwest::StatusCode, String),
    SseStreamError(sse_stream::Error),
    CacheMiss,
    CacheError(String),
}

impl From<reqwest::Error> for BmcError {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}

impl CacheableError for BmcError {
    fn is_cached(&self) -> bool {
        match self {
            Self::InvalidResponse(status, _) => status == &reqwest::StatusCode::NOT_MODIFIED,
            _ => false,
        }
    }

    fn cache_miss() -> Self {
        Self::CacheMiss
    }

    fn cache_error(reason: String) -> Self {
        Self::CacheError(reason)
    }
}

#[allow(clippy::absolute_paths)]
impl std::fmt::Display for BmcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReqwestError(e) => write!(f, "HTTP client error: {e:?}"),
            Self::InvalidResponse(status, text) => {
                write!(
                    f,
                    "Invalid HTTP response - status: {} text: {}",
                    status, text
                )
            }
            Self::CacheMiss => write!(f, "Resource not found in cache"),
            Self::CacheError(r) => write!(f, "Error occurred in cache {r:?}"),
            Self::JsonError(e) => write!(
                f,
                "JSON deserialization error at line {} column {} path {}: {e}",
                e.inner().line(),
                e.inner().column(),
                e.path(),
            ),
            Self::SseStreamError(e) => write!(f, "SSE stream decode error: {e}"),
        }
    }
}

#[allow(clippy::absolute_paths)]
impl std::error::Error for BmcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReqwestError(e) => Some(e),
            Self::JsonError(e) => Some(e.inner()),
            Self::SseStreamError(e) => Some(e),
            _ => None,
        }
    }
}

/// Configuration parameters for the reqwest HTTP client.
///
/// This struct allows customizing various aspects of the reqwest client behavior,
/// including timeouts, TLS settings, and connection pooling.
///
/// # Examples
///
/// ```rust
/// use nv_redfish_bmc_http::reqwest::ClientParams;
/// use std::time::Duration;
///
/// let params = ClientParams::new()
///     .timeout(Duration::from_secs(30))
///     .connect_timeout(Duration::from_secs(10))
///     .user_agent("MyApp/1.0")
///     .accept_invalid_certs(true);
/// ```
#[derive(Debug, Clone)]
pub struct ClientParams {
    /// HTTP request timeout
    pub timeout: Option<Duration>,
    /// TCP connection timeout
    pub connect_timeout: Option<Duration>,
    /// User-Agent header value
    pub user_agent: Option<String>,
    /// Whether to accept invalid TLS certificates
    pub accept_invalid_certs: bool,
    /// Maximum number of HTTP redirects to follow
    pub max_redirects: Option<usize>,
    /// TCP keep-alive timeout
    pub tcp_keepalive: Option<Duration>,
    /// Connection pool idle timeout
    pub pool_idle_timeout: Option<Duration>,
    /// Maximum idle connections per host
    pub pool_max_idle_per_host: Option<usize>,
    /// List of default headers, added to every request
    pub default_headers: Option<HeaderMap>,
    /// Forces use of rust TLS, enabled by default
    pub use_rust_tls: bool,
}

impl Default for ClientParams {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(120)),
            connect_timeout: Some(Duration::from_secs(5)),
            user_agent: Some("nv-redfish/v1".to_string()),
            accept_invalid_certs: false,
            max_redirects: Some(10),
            tcp_keepalive: Some(Duration::from_secs(60)),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            pool_max_idle_per_host: Some(1),
            default_headers: None,
            use_rust_tls: true,
        }
    }
}

impl ClientParams {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    #[must_use]
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    #[must_use]
    pub const fn accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    #[must_use]
    pub const fn max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = Some(max);
        self
    }

    #[must_use]
    pub const fn tcp_keepalive(mut self, keepalive: Duration) -> Self {
        self.tcp_keepalive = Some(keepalive);
        self
    }

    #[must_use]
    pub const fn pool_max_idle_per_host(mut self, pool_max_idle_per_host: usize) -> Self {
        self.pool_max_idle_per_host = Some(pool_max_idle_per_host);
        self
    }

    #[must_use]
    pub const fn idle_timeout(mut self, pool_idle_timeout: Duration) -> Self {
        self.pool_idle_timeout = Some(pool_idle_timeout);
        self
    }

    #[must_use]
    pub const fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    #[must_use]
    pub fn default_headers(mut self, default_headers: HeaderMap) -> Self {
        self.default_headers = Some(default_headers);
        self
    }
}

/// HTTP client implementation using the reqwest library.
///
/// This provides a concrete implementation of [`HttpClient`] using the popular
/// reqwest HTTP client library. It supports all standard HTTP features including
/// TLS, authentication, and connection pooling.
///
/// # Examples
///
/// ```rust,no_run
/// use nv_redfish_bmc_http::HttpBmc;
/// use nv_redfish_bmc_http::reqwest::Client;
/// use nv_redfish_bmc_http::CacheSettings;
/// use nv_redfish_bmc_http::BmcCredentials;
/// use nv_redfish_bmc_http::reqwest::ClientParams;
/// use std::time::Duration;
/// use url::Url;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create with default settings
/// let client = Client::new()?;
///
/// // Or with custom parameters
/// let params = ClientParams::new().timeout(Duration::from_secs(60));
/// let client = Client::with_params(params)?;
///
/// // Use with HttpBmc
/// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
/// let endpoint = Url::parse("https://192.168.1.100")?;
/// let bmc = HttpBmc::new(client, endpoint, credentials, CacheSettings::default());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
}

#[allow(clippy::missing_errors_doc)]
#[allow(clippy::absolute_paths)]
impl Client {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_params(ClientParams::default())
    }

    pub fn with_params(params: ClientParams) -> Result<Self, reqwest::Error> {
        let mut builder = reqwest::Client::builder();

        if params.use_rust_tls {
            builder = builder.use_rustls_tls();
        }

        if let Some(timeout) = params.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(connect_timeout) = params.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        if let Some(user_agent) = params.user_agent {
            builder = builder.user_agent(user_agent);
        }

        if params.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some(max_redirects) = params.max_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::limited(max_redirects));
        }

        if let Some(keepalive) = params.tcp_keepalive {
            builder = builder.tcp_keepalive(keepalive);
        }

        if let Some(idle_timeout) = params.pool_idle_timeout {
            builder = builder.pool_idle_timeout(idle_timeout);
        }

        if let Some(max_idle) = params.pool_max_idle_per_host {
            builder = builder.pool_max_idle_per_host(max_idle);
        }

        if let Some(default_headers) = params.default_headers {
            builder = builder.default_headers(default_headers);
        }

        Ok(Self {
            client: builder.build()?,
        })
    }

    #[must_use]
    pub const fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Client {
    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T, BmcError>
    where
        T: DeserializeOwned,
    {
        if !response.status().is_success() {
            return Err(BmcError::InvalidResponse(
                response.status(),
                response.text().await.unwrap_or_else(|_| "<no data>".into()),
            ));
        }

        let etag_header = response.headers().get("etag").cloned();

        let mut value: serde_json::Value = response.json().await.map_err(BmcError::ReqwestError)?;

        if let Some(header) = etag_header {
            if let Ok(etag_value) = header.to_str() {
                if let Some(obj) = value.as_object_mut() {
                    let etag_value = serde_json::Value::String(etag_value.to_string());

                    // Handles both absent and null values
                    obj.entry("@odata.etag")
                        .and_modify(|v| *v = etag_value.clone())
                        .or_insert(etag_value);
                }
            }
        }

        serde_path_to_error::deserialize(value).map_err(BmcError::JsonError)
    }
}

impl HttpClient for Client {
    type Error = BmcError;

    async fn get<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        etag: Option<ODataETag>,
        custom_headers: &HeaderMap,
    ) -> Result<T, Self::Error>
    where
        T: DeserializeOwned,
    {
        let mut request = self
            .client
            .get(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .headers(custom_headers.clone());

        if let Some(etag) = etag {
            request = request.header(header::IF_NONE_MATCH, etag.to_string());
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    async fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<T, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let response = self
            .client
            .post(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .headers(custom_headers.clone())
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn patch<B, T>(
        &self,
        url: Url,
        etag: ODataETag,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<T, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let mut request = self
            .client
            .patch(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .headers(custom_headers.clone());

        request = request.header(header::IF_MATCH, etag.to_string());

        let response = request.json(body).send().await?;
        self.handle_response(response).await
    }

    async fn delete(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<Empty, Self::Error> {
        let response = self
            .client
            .delete(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .headers(custom_headers.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BmcError::InvalidResponse(
                response.status(),
                response.text().await.unwrap_or_else(|_| "<no data>".into()),
            ));
        }

        Ok(Empty {})
    }

    async fn sse<T: Sized + for<'a> serde::Deserialize<'a> + Send + 'static>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<BoxTryStream<T, Self::Error>, Self::Error> {
        let response = self
            .client
            .get(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .headers(custom_headers.clone())
            .header(header::ACCEPT, "text/event-stream")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BmcError::InvalidResponse(
                response.status(),
                response.text().await.unwrap_or_else(|_| "<no data>".into()),
            ));
        }

        let stream = sse_stream::SseStream::from_byte_stream(response.bytes_stream()).filter_map(
            |event| async move {
                match event {
                    Err(err) => Some(Err(BmcError::SseStreamError(err))),
                    Ok(sse) => sse.data.map(|data| {
                        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_str(
                            &data,
                        ))
                        .map_err(BmcError::JsonError)
                    }),
                }
            },
        );

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cacheable_error_trait() {
        let mock_response = reqwest::Response::from(
            http::Response::builder()
                .status(304)
                .body("")
                .expect("Valid empty body"),
        );
        let error = BmcError::InvalidResponse(mock_response.status(), "".into());
        assert!(error.is_cached());

        let cache_miss = BmcError::CacheMiss;
        assert!(!cache_miss.is_cached());

        let created_miss = BmcError::cache_miss();
        assert!(matches!(created_miss, BmcError::CacheMiss));
    }
}
