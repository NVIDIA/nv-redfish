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

//! Implementation of [`HttpClient`] trait using reqwest crate.

use std::error::Error as StdErr;
use std::fmt;
use std::time::Duration;

use crate::BmcCredentials;
use crate::CacheableError;
use crate::HttpClient;
use crate::MultipartUpdateRequest;
use crate::schema::redfish::redfish_error::RedfishError;

use futures_util::StreamExt as _;
use http::HeaderMap;
use http::header;
use nv_redfish_core::AsyncTask;
use nv_redfish_core::BoxTryStream;
use nv_redfish_core::DataStream;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use nv_redfish_core::OemMultipartPart;
use nv_redfish_core::SessionCreateResponse;
use nv_redfish_core::UploadReader;
use reqwest::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use reqwest::multipart::Form;
use reqwest::multipart::Part;
use reqwest::redirect::Policy as RedirectPolicy;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio_util::compat::FuturesAsyncReadCompatExt as _;
use tokio_util::io::ReaderStream;
use url::Url;

/// Errors of reqwest implementation of the HTTP trait.
#[derive(Debug)]
pub enum BmcError {
    /// Direct mapping of underlying reqwest error.
    ReqwestError(reqwest::Error),
    /// JSON to model deserialize error with path tracking.
    JsonError(serde_path_to_error::Error<serde_json::Error>),
    /// Unexpected HTTP response.
    InvalidResponse {
        /// URL in request that caused error.
        url: url::Url,
        /// Returned status.
        status: reqwest::StatusCode,
        /// Text in the response.
        text: String,
    },
    /// SSE stream error.
    SseStreamError(sse_stream::Error),
    /// No resource found in cache.
    CacheMiss,
    /// HTTP cache error.
    CacheError(String),
    /// JSON deserialization error.
    DecodeError(serde_json::Error),
    /// JSON serialization error.
    EncodeError(serde_json::Error),
    /// Invalid request error - data in the request didn't pass validation.
    InvalidRequest(String),
}

impl From<reqwest::Error> for BmcError {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}

impl CacheableError for BmcError {
    fn is_cached(&self) -> bool {
        match self {
            Self::InvalidResponse { status, .. } => status == &reqwest::StatusCode::NOT_MODIFIED,
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

impl fmt::Display for BmcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReqwestError(e) => write!(f, "HTTP client error: {e:?}"),
            Self::InvalidResponse { url, status, text } => {
                write!(
                    f,
                    "Invalid HTTP response - url: {url} status: {status} text: {text}"
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
            Self::DecodeError(e) => write!(f, "JSON Decode error: {e}"),
            Self::EncodeError(e) => write!(f, "JSON Encode error: {e}"),
            Self::InvalidRequest(e) => write!(f, "Invalid request: {e}"),
        }
    }
}

impl StdErr for BmcError {
    fn source(&self) -> Option<&(dyn StdErr + 'static)> {
        match self {
            Self::ReqwestError(e) => Some(e),
            Self::JsonError(e) => Some(e.inner()),
            Self::SseStreamError(e) => Some(e),
            Self::DecodeError(e) | Self::EncodeError(e) => Some(e),
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
    /// Creates new client parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// See: [`reqwest::ClientBuilder::timeout`].
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// See: [`reqwest::ClientBuilder::connect_timeout`].
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// See: [`reqwest::ClientBuilder::user_agent`].
    #[must_use]
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// See: [`reqwest::ClientBuilder::danger_accept_invalid_certs`].
    #[must_use]
    pub const fn accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    /// See: [`reqwest::ClientBuilder::redirect`].
    #[must_use]
    pub const fn max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = Some(max);
        self
    }

    /// See: [`reqwest::ClientBuilder::tcp_keepalive`].
    #[must_use]
    pub const fn tcp_keepalive(mut self, keepalive: Duration) -> Self {
        self.tcp_keepalive = Some(keepalive);
        self
    }

    /// See: [`reqwest::ClientBuilder::pool_max_idle_per_host`].
    #[must_use]
    pub const fn pool_max_idle_per_host(mut self, pool_max_idle_per_host: usize) -> Self {
        self.pool_max_idle_per_host = Some(pool_max_idle_per_host);
        self
    }

    /// See: [`reqwest::ClientBuilder::pool_idle_timeout`].
    #[must_use]
    pub const fn idle_timeout(mut self, pool_idle_timeout: Duration) -> Self {
        self.pool_idle_timeout = Some(pool_idle_timeout);
        self
    }

    /// Clears timeout for this client.
    #[must_use]
    pub const fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// See: [`reqwest::ClientBuilder::default_headers`].
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
#[derive(Clone)]
pub struct Client {
    client: ReqwestClient,
}

impl Client {
    /// Create client with default [`ClientParams`].
    ///
    /// # Errors
    ///
    /// Internally it builds [`reqwest::ClientBuilder::build`]. This function
    /// transparently passes errors of this call to caller.
    pub fn new() -> Result<Self, ReqwestError> {
        Self::with_params(ClientParams::default())
    }

    /// Build client from parameters.
    ///
    /// # Errors
    ///
    /// Internally it builds [`reqwest::ClientBuilder::build`]. This function
    /// transparently passes errors of this call to caller.
    pub fn with_params(params: ClientParams) -> Result<Self, reqwest::Error> {
        let mut builder = ReqwestClient::builder();

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
            builder = builder.redirect(RedirectPolicy::limited(max_redirects));
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

    /// Use pre-built [`reqwest::Client`] as internal client.
    #[must_use]
    pub const fn with_client(client: ReqwestClient) -> Self {
        Self { client }
    }
}

impl Client {
    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T, BmcError>
    where
        T: DeserializeOwned,
    {
        if !response.status().is_success() {
            return Err(BmcError::InvalidResponse {
                url: response.url().clone(),
                status: response.status(),
                text: response.text().await.unwrap_or_else(|_| "<no data>".into()),
            });
        }

        let headers = response.headers().clone();

        let etag_header = etag_from_headers(&headers);

        let mut value: serde_json::Value = response.json().await.map_err(BmcError::ReqwestError)?;

        if let Some(etag) = etag_header {
            inject_etag(&etag, &mut value);
        }

        serde_path_to_error::deserialize(value).map_err(BmcError::JsonError)
    }

    async fn handle_modification_response<T>(
        &self,
        response: reqwest::Response,
    ) -> Result<ModificationResponse<T>, BmcError>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let status = response.status();
        let url = response.url().clone();
        let headers = response.headers().clone();
        if !status.is_success() {
            return Err(BmcError::InvalidResponse {
                url,
                status,
                text: response.text().await.unwrap_or_else(|_| "<no data>".into()),
            });
        }

        let etag = etag_from_headers(&headers);
        let location = location_from_headers(&headers);

        match status {
            reqwest::StatusCode::NO_CONTENT => Ok(ModificationResponse::Empty),
            reqwest::StatusCode::ACCEPTED => {
                let Some(task_monitor_id) = location else {
                    return Err(BmcError::InvalidResponse {
                        url,
                        status,
                        text: String::from("202 Accepted without Location header"),
                    });
                };
                Ok(ModificationResponse::Task(AsyncTask {
                    id: task_monitor_id,
                    retry_after_secs: retry_after_from_headers(&headers),
                }))
            }
            reqwest::StatusCode::OK | reqwest::StatusCode::CREATED => {
                let bytes = response.bytes().await.map_err(BmcError::ReqwestError)?;
                if !bytes.is_empty() {
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(BmcError::DecodeError)?;
                    let mut value = value;

                    if value.get("@odata.id").is_some() {
                        if let Some(etag) = etag {
                            inject_etag(&etag, &mut value);
                        }
                    }

                    // Non-empty 200/201 bodies are typed responses selected by the caller.
                    //
                    // - These bodies are not required to be Redfish resources with @odata.id.
                    //
                    // - DSP0266 POST actions with no response body may still return "an error
                    //   response, with a message that indicates success".
                    return match serde_path_to_error::deserialize(&value) {
                        // Non-empty 200/201 body matched the caller-selected type.
                        Ok(entity) => Ok(ModificationResponse::Entity(entity)),
                        Err(err) => {
                            if expects_no_response_body::<T>()
                                && is_redfish_success_response(&value)
                            {
                                // No-response action returned a Redfish success envelope.
                                Ok(ModificationResponse::Empty)
                            } else {
                                // The response was successful JSON, but it did
                                // not match the caller-selected response type.
                                Err(BmcError::JsonError(err))
                            }
                        }
                    };
                }

                if let Some(location) = location {
                    let value = serde_json::json!({ "@odata.id": location });
                    return serde_path_to_error::deserialize(value)
                        .map(ModificationResponse::Entity)
                        .map_err(BmcError::JsonError);
                }

                Ok(ModificationResponse::Empty)
            }
            _ => Err(BmcError::InvalidResponse {
                url,
                status,
                text: format!("Unexpected successful status code: {status}"),
            }),
        }
    }

    async fn handle_session_response<T>(
        &self,
        response: reqwest::Response,
    ) -> Result<SessionCreateResponse<T>, BmcError>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let status = response.status();
        let url = response.url().clone();
        let headers = response.headers().clone();
        if !status.is_success() {
            return Err(BmcError::InvalidResponse {
                url,
                status,
                text: response.text().await.unwrap_or_else(|_| "<no data>".into()),
            });
        }

        let Some(auth_token) = auth_token_from_headers(&headers) else {
            return Err(BmcError::InvalidResponse {
                url,
                status,
                text: String::from("session creation response missing X-Auth-Token header"),
            });
        };
        let Some(location) = location_from_headers(&headers) else {
            return Err(BmcError::InvalidResponse {
                url,
                status,
                text: String::from("session creation response missing Location header"),
            });
        };

        match status {
            reqwest::StatusCode::OK | reqwest::StatusCode::CREATED => {
                let etag = etag_from_headers(&headers);
                let bytes = response.bytes().await.map_err(BmcError::ReqwestError)?;
                if bytes.is_empty() {
                    return Err(BmcError::InvalidResponse {
                        url,
                        status,
                        text: String::from("session creation response missing entity body"),
                    });
                }

                let mut value: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(BmcError::DecodeError)?;
                if let Some(etag) = etag {
                    inject_etag(&etag, &mut value);
                }
                let entity =
                    serde_path_to_error::deserialize(value).map_err(BmcError::JsonError)?;

                Ok(SessionCreateResponse {
                    entity,
                    auth_token,
                    location,
                })
            }
            reqwest::StatusCode::ACCEPTED => Err(BmcError::InvalidResponse {
                url,
                status,
                text: String::from("session creation returned 202 Accepted without session entity"),
            }),
            reqwest::StatusCode::NO_CONTENT => Err(BmcError::InvalidResponse {
                url,
                status,
                text: String::from("session creation returned 204 No Content"),
            }),
            _ => Err(BmcError::InvalidResponse {
                url,
                status,
                text: format!("Unexpected successful status code for session creation: {status}"),
            }),
        }
    }
}

fn location_from_headers(headers: &HeaderMap) -> Option<ODataId> {
    headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(|raw| {
            Url::parse(raw).map_or_else(
                |_| raw.to_string().into(),
                |url| {
                    let mut path = url.path().to_string();
                    if let Some(query) = url.query() {
                        path.push('?');
                        path.push_str(query);
                    }
                    path.into()
                },
            )
        })
}

fn auth_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-auth-token")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn etag_from_headers(headers: &HeaderMap) -> Option<ODataETag> {
    headers
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(|v| v.to_string().into())
}

fn retry_after_from_headers(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
}

fn inject_etag(etag: &ODataETag, body: &mut serde_json::Value) {
    if let Some(obj) = body.as_object_mut() {
        let etag_value = serde_json::Value::String(etag.to_string());

        // Handles both absent and null values
        obj.entry("@odata.etag")
            .and_modify(|v| *v = etag_value.clone())
            .or_insert(etag_value);
    }
}

/// DSP0266 7.11, Table 10 allows actions without response bodies to return
/// an error-shaped success body. Only that body should become Empty.
#[inline]
fn is_redfish_success_response(value: &serde_json::Value) -> bool {
    let Ok(response) = <RedfishError as serde::Deserialize>::deserialize(value) else {
        return false;
    };

    let code = response.error.code.as_str();
    let message = code.rsplit_once('.').map_or(code, |(_, message)| message);

    matches!(message, "Success" | "Created" | "NoOperation")
}

/// Distinguishes no-response callers like () from typed response callers.
/// The same endpoint can be called with different R types, so only callers
/// whose R can represent no body may map a success envelope to Empty.
#[inline]
fn expects_no_response_body<T>() -> bool
where
    T: DeserializeOwned,
{
    serde_json::from_value::<T>(serde_json::Value::Null).is_ok()
}

fn auth_headers(
    request: reqwest::RequestBuilder,
    credentials: &BmcCredentials,
) -> reqwest::RequestBuilder {
    match credentials {
        BmcCredentials::UsernamePassword { username, password } => {
            request.basic_auth(username, password.as_ref())
        }
        BmcCredentials::Token { token } => request.header("X-Auth-Token", token),
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
        let mut request =
            auth_headers(self.client.get(url), credentials).headers(custom_headers.clone());

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
    ) -> Result<ModificationResponse<T>, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let response = auth_headers(self.client.post(url), credentials)
            .headers(custom_headers.clone())
            .json(body)
            .send()
            .await?;

        self.handle_modification_response(response).await
    }

    async fn post_session<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<SessionCreateResponse<T>, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let response = auth_headers(self.client.post(url), credentials)
            .headers(custom_headers.clone())
            .json(body)
            .send()
            .await?;

        self.handle_session_response(response).await
    }

    async fn patch<B, T>(
        &self,
        url: Url,
        etag: ODataETag,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<ModificationResponse<T>, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let mut request =
            auth_headers(self.client.patch(url), credentials).headers(custom_headers.clone());

        request = request.header(header::IF_MATCH, etag.to_string());

        let response = request.json(body).send().await?;
        self.handle_modification_response(response).await
    }

    async fn delete<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<ModificationResponse<T>, Self::Error>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let response = auth_headers(self.client.delete(url), credentials)
            .headers(custom_headers.clone())
            .send()
            .await?;

        self.handle_modification_response(response).await
    }

    async fn post_multipart_update<U, V, T>(
        &self,
        url: Url,
        update_request: MultipartUpdateRequest<'_, U, V>,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<ModificationResponse<T>, Self::Error>
    where
        U: UploadReader,
        T: DeserializeOwned + Send + Sync,
        V: Serialize + Send + Sync,
    {
        let MultipartUpdateRequest {
            update_parameters,
            update_stream,
            oem_parts,
            upload_timeout,
        } = update_request;

        // First, check if all OEM parts have valid names.
        for part in &oem_parts {
            if !part.is_name_valid() {
                return Err(BmcError::InvalidRequest(format!(
                    "OEM part's name `{}` is invalid",
                    part.name
                )));
            }
        }

        let stream_part = build_stream_part(update_stream, "application/octet-stream")?;
        let update_parameters_part = build_update_parameters_part(update_parameters)?;

        let mut form = Form::new()
            .part("UpdateParameters", update_parameters_part)
            .part("UpdateFile", stream_part);

        for part in oem_parts {
            let (name, part) = build_oem_part(part)?;
            form = form.part(name, part);
        }

        let response = auth_headers(self.client.post(url), credentials)
            .headers(custom_headers.clone())
            .multipart(form)
            .timeout(upload_timeout)
            .send()
            .await?;

        self.handle_modification_response(response).await
    }

    async fn sse<T: Send + Sized + for<'de> serde::Deserialize<'de>>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> Result<BoxTryStream<T, Self::Error>, Self::Error> {
        let response = auth_headers(self.client.get(url), credentials)
            .headers(custom_headers.clone())
            .header(header::ACCEPT, "text/event-stream")
            .timeout(Duration::MAX)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BmcError::InvalidResponse {
                url: response.url().clone(),
                status: response.status(),
                text: response.text().await.unwrap_or_else(|_| "<no data>".into()),
            });
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

fn build_update_parameters_part<V>(update_parameters: &V) -> Result<Part, BmcError>
where
    V: Serialize + Send + Sync,
{
    Part::bytes(serde_json::to_vec(update_parameters).map_err(BmcError::EncodeError)?)
        .mime_str("application/json")
        .map_err(BmcError::ReqwestError)
}

fn build_stream_part<U>(stream: DataStream<U>, content_type: &'static str) -> Result<Part, BmcError>
where
    U: UploadReader,
{
    let DataStream {
        name,
        reader,
        content_length,
    } = stream;

    let body = reqwest::Body::wrap_stream(ReaderStream::new(reader.compat()));
    let part = match content_length {
        Some(length) => Part::stream_with_length(body, length),
        None => Part::stream(body),
    };

    part.file_name(name)
        .mime_str(content_type)
        .map_err(BmcError::ReqwestError)
}

fn build_oem_part(part: OemMultipartPart) -> Result<(String, Part), BmcError> {
    let OemMultipartPart {
        name,
        reader,
        content_type,
        content_length,
    } = part;

    let body = reqwest::Body::wrap_stream(ReaderStream::new(reader.compat()));

    let mut part = match content_length {
        Some(length) => Part::stream_with_length(body, length),
        None => Part::stream(body),
    };

    if let Some(content_type) = content_type {
        part = part.mime_str(&content_type)?;
    }

    Ok((name, part))
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::*;

    use futures_util::io::Cursor;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[derive(serde::Serialize)]
    struct MultipartParameters {
        #[serde(rename = "ForceUpdate")]
        force_update: bool,

        #[serde(rename = "Targets")]
        targets: Vec<String>,
    }

    #[test]
    fn test_cacheable_error_trait() {
        let mock_response = reqwest::Response::from(
            http::Response::builder()
                .status(304)
                .body("")
                .expect("Valid empty body"),
        );
        let error = BmcError::InvalidResponse {
            url: "http://example.com/redfish/v1".parse().unwrap(),
            status: mock_response.status(),
            text: "".into(),
        };
        assert!(error.is_cached());

        let cache_miss = BmcError::CacheMiss;
        assert!(!cache_miss.is_cached());

        let created_miss = BmcError::cache_miss();
        assert!(matches!(created_miss, BmcError::CacheMiss));
    }

    #[tokio::test]
    async fn test_multipart_form_fails_oem_validation() -> Result<(), Box<dyn StdError>> {
        let mock_server = MockServer::start().await;
        let upload_path = "/redfish/v1/UpdateService/update-multipart";
        let task_path = "/redfish/v1/TaskService/Tasks/42";

        Mock::given(method("POST"))
            .and(path(upload_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("X-Upload-Mode", "multipart"))
            .and(|request: &Request| {
                multipart_body_contains(request, "firmware.bin", "firmware-bytes")
            })
            .respond_with(
                ResponseTemplate::new(202)
                    .insert_header("Location", format!("https://bmc.example{task_path}"))
                    .insert_header("Retry-After", "15"),
            )
            .expect(0)
            .mount(&mock_server)
            .await;

        let params = MultipartParameters {
            force_update: true,
            targets: vec!["/redfish/v1/Systems/1".to_string()],
        };

        let mut custom_headers = HeaderMap::new();
        custom_headers.insert("X-Upload-Mode", http::HeaderValue::from_static("multipart"));

        let client = Client::new()?;
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());

        //
        // Invalid OEM part.
        //
        let update_stream =
            DataStream::new("firmware.bin", Cursor::new(b"firmware-bytes".to_vec()))
                .with_content_length(14);

        // Construction error - fails name validation.
        let r = OemMultipartPart::new("oemNvidia", Cursor::new(br#"{"Mode":"Rms"}"#.to_vec()));
        assert!(r.is_err());

        let mut invalid_oem_part =
            OemMultipartPart::new("OemNvidia", Cursor::new(br#"{"Mode":"Rms"}"#.to_vec()))?
                .with_content_type("application/json");
        invalid_oem_part.name = "oemNvidia".to_string();

        let update_request = MultipartUpdateRequest {
            update_parameters: &params,
            update_stream,
            oem_parts: vec![invalid_oem_part],
            upload_timeout: Duration::from_secs(600),
        };

        let response = client
            .post_multipart_update::<_, _, serde_json::Value>(
                Url::parse(&format!("{}{upload_path}", mock_server.uri()))?,
                update_request,
                &credentials,
                &custom_headers,
            )
            .await;

        assert!(response.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_multipart_form_sends_parts_and_returns_task() -> Result<(), Box<dyn StdError>> {
        let mock_server = MockServer::start().await;
        let upload_path = "/redfish/v1/UpdateService/update-multipart";
        let task_path = "/redfish/v1/TaskService/Tasks/42";

        Mock::given(method("POST"))
            .and(path(upload_path))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("X-Upload-Mode", "multipart"))
            .and(|request: &Request| {
                multipart_body_contains(request, "firmware.bin", "firmware-bytes")
            })
            .respond_with(
                ResponseTemplate::new(202)
                    .insert_header("Location", format!("https://bmc.example{task_path}"))
                    .insert_header("Retry-After", "15"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let params = MultipartParameters {
            force_update: true,
            targets: vec!["/redfish/v1/Systems/1".to_string()],
        };

        let mut custom_headers = HeaderMap::new();
        custom_headers.insert("X-Upload-Mode", http::HeaderValue::from_static("multipart"));

        let client = Client::new()?;
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());

        let update_stream =
            DataStream::new("firmware.bin", Cursor::new(b"firmware-bytes".to_vec()))
                .with_content_length(14);

        let update_request = MultipartUpdateRequest {
            update_parameters: &params,
            update_stream,
            oem_parts: vec![
                OemMultipartPart::new("OemNvidia", Cursor::new(br#"{"Mode":"Rms"}"#.to_vec()))?
                    .with_content_type("application/json"),
            ],
            upload_timeout: Duration::from_secs(600),
        };

        let response = client
            .post_multipart_update::<_, _, serde_json::Value>(
                Url::parse(&format!("{}{upload_path}", mock_server.uri()))?,
                update_request,
                &credentials,
                &custom_headers,
            )
            .await?;

        let ModificationResponse::Task(task) = response else {
            return Err(String::from("expected task response").into());
        };

        assert_eq!(task.id.to_string(), task_path);
        assert_eq!(task.retry_after_secs, Some(15));

        Ok(())
    }

    fn multipart_body_contains(request: &Request, file_name: &str, file_body: &str) -> bool {
        let Some(content_type) = request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };

        let body = String::from_utf8_lossy(&request.body);

        content_type.starts_with("multipart/form-data; boundary=")
            && body.contains("name=\"UpdateParameters\"")
            && body.contains("Content-Type: application/json")
            && body.contains("\"ForceUpdate\":true")
            && body.contains("\"Targets\":[\"/redfish/v1/Systems/1\"]")
            && body.contains("name=\"UpdateFile\"")
            && body.contains("Content-Type: application/octet-stream")
            && body.contains(&format!("filename=\"{file_name}\""))
            && body.contains("name=\"OemNvidia\"")
            && !body.contains("name=\"OemNvidia\"; filename=")
            && body.contains("{\"Mode\":\"Rms\"}")
            && body.contains(file_body)
    }
}
