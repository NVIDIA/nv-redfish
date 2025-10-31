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

pub mod cache;
pub mod credentials;

use crate::cache::TypeErasedCarCache;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Action;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::FilterQuery;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error as StdError,
    future::Future,
    sync::{Arc, RwLock},
};
use url::Url;

#[cfg(feature = "reqwest")]
use nv_redfish_core::Empty;

#[cfg(feature = "reqwest")]
use std::time::Duration;

#[doc(inline)]
pub use credentials::BmcCredentials;

pub trait HttpClient: Send + Sync {
    type Error: Send + StdError;

    /// Perform an HTTP GET request with optional conditional headers.
    fn get<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        etag: Option<ODataETag>,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP POST request.
    fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP PATCH request.
    fn patch<B, T>(
        &self,
        url: Url,
        etag: ODataETag,
        body: &B,
        credentials: &BmcCredentials,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP DELETE request.
    fn delete(
        &self,
        url: Url,
        credentials: &BmcCredentials,
    ) -> impl Future<Output = Result<Empty, Self::Error>> + Send;
}

/// HTTP-based BMC implementation that wraps an [`HttpClient`].
///
/// This struct combines an HTTP client with BMC endpoint information and credentials
/// to provide a complete Redfish client implementation. It implements the [`Bmc`] trait
/// to provide standardized access to Redfish services.
///
/// # Type Parameters
///
/// * `C` - The HTTP client implementation to use
///
/// # Examples
///
/// ```rust,no_run
/// use nv_redfish_bmc_http::{HttpBmc, ReqwestClient};
/// use nv_redfish_bmc_http::BmcCredentials;
/// use nv_redfish_core::{Bmc, ODataId};
/// use url::Url;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
/// let http_client = ReqwestClient::new()?;
/// let endpoint = Url::parse("https://192.168.1.100")?;
///
/// let bmc = HttpBmc::new(http_client, endpoint, credentials);
/// # Ok(())
/// # }
/// ```
pub struct HttpBmc<C: HttpClient> {
    client: C,
    redfish_endpoint: RedfishEndpoint,
    credentials: BmcCredentials,
    cache: RwLock<TypeErasedCarCache<ODataId>>,
    etags: RwLock<HashMap<ODataId, ODataETag>>,
}

impl<C: HttpClient> HttpBmc<C>
where
    C::Error: CacheableError,
{
    /// Create a new HTTP-based BMC client with ETag-based caching.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client implementation to use for requests
    /// * `redfish_endpoint` - The base URL of the Redfish service (e.g., `https://192.168.1.100`)
    /// * `credentials` - Authentication credentials for the BMC
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use nv_redfish_bmc_http::{HttpBmc, ReqwestClient};
    /// use nv_redfish_bmc_http::BmcCredentials;
    /// use url::Url;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
    /// let http_client = ReqwestClient::new()?;
    /// let endpoint = Url::parse("https://192.168.1.100")?;
    ///
    /// let bmc = HttpBmc::new(http_client, endpoint, credentials);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(client: C, redfish_endpoint: Url, credentials: BmcCredentials) -> Self {
        Self {
            client,
            redfish_endpoint: RedfishEndpoint::from(redfish_endpoint),
            credentials,
            cache: RwLock::new(TypeErasedCarCache::new(1000)),
            etags: RwLock::new(HashMap::new()),
        }
    }
}

/// A tagged type representing a Redfish endpoint URL.
///
/// Provides convenient conversion methods to build endpoint URLs from `ODataId` paths.
#[derive(Debug, Clone)]
pub struct RedfishEndpoint {
    base_url: Url,
}

impl RedfishEndpoint {
    /// Create a new `RedfishEndpoint` from a base URL
    #[must_use]
    pub const fn new(base_url: Url) -> Self {
        Self { base_url }
    }

    /// Convert a path to a full Redfish endpoint URL
    #[must_use]
    pub fn with_path(&self, path: &str) -> Url {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url
    }

    /// Convert a path to a full Redfish endpoint URL with query parameters
    #[must_use]
    pub fn with_path_and_query(&self, path: &str, query: &str) -> Url {
        let mut url = self.with_path(path);
        url.set_query(Some(query));
        url
    }
}

impl From<Url> for RedfishEndpoint {
    fn from(url: Url) -> Self {
        Self::new(url)
    }
}

impl From<&RedfishEndpoint> for Url {
    fn from(endpoint: &RedfishEndpoint) -> Self {
        endpoint.base_url.clone()
    }
}

/// Trait for errors that can indicate whether they represent a cached response
/// and provide a way to create cache-related errors.
pub trait CacheableError {
    /// Returns true if this error indicates the resource should be served from cache.
    /// Typically true for HTTP 304 Not Modified responses.
    fn is_cached(&self) -> bool;

    /// Create an error for when cached data is requested but not available.
    fn cache_miss() -> Self;

    /// Cache error
    fn cache_error(reason: String) -> Self;
}

impl<C: HttpClient> HttpBmc<C>
where
    C::Error: CacheableError + StdError + Send + Sync,
{
    /// Perform a GET request with `ETag` caching support
    ///
    /// This handles:
    /// - Retrieving cached `ETag` before request
    /// - Sending conditional GET with If-None-Match
    /// - Handling 304 Not Modified responses from cache
    /// - Updating cache and `ETag` storage on success
    #[allow(clippy::significant_drop_tightening)]
    async fn get_with_cache<
        T: EntityTypeRef + Sized + for<'de> Deserialize<'de> + 'static + Send + Sync,
    >(
        &self,
        endpoint_url: Url,
        id: &ODataId,
    ) -> Result<Arc<T>, C::Error> {
        // Retrieve cached etag
        let etag: Option<ODataETag> = {
            let etags = self
                .etags
                .read()
                .map_err(|e| C::Error::cache_error(e.to_string()))?;
            etags.get(id).cloned()
        };

        // Perform GET request
        match self
            .client
            .get::<T>(endpoint_url, &self.credentials, etag)
            .await
        {
            Ok(response) => {
                let entity = Arc::new(response);

                // Update cache if entity has etag
                if let Some(etag) = entity.etag() {
                    let mut cache = self
                        .cache
                        .write()
                        .map_err(|e| C::Error::cache_error(e.to_string()))?;

                    let mut etags = self
                        .etags
                        .write()
                        .map_err(|e| C::Error::cache_error(e.to_string()))?;

                    if let Some(ret) = cache.put_typed(id.clone(), Arc::clone(&entity)) {
                        etags.remove(ret.id());
                    }
                    etags.insert(id.clone(), etag.clone());
                }
                Ok(entity)
            }
            Err(e) => {
                // Handle 304 Not Modified - return from cache
                if e.is_cached() {
                    let mut cache = self
                        .cache
                        .write()
                        .map_err(|e| C::Error::cache_error(e.to_string()))?;
                    cache
                        .get_typed::<Arc<T>>(id)
                        .cloned()
                        .ok_or_else(C::Error::cache_miss)
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl<C: HttpClient> Bmc for HttpBmc<C>
where
    C::Error: CacheableError + StdError + Send + Sync,
{
    type Error = C::Error;

    async fn get<T: EntityTypeRef + Sized + for<'de> Deserialize<'de> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        self.get_with_cache(endpoint_url, id).await
    }

    async fn expand<T: Expandable + Send + Sync + 'static>(
        &self,
        id: &ODataId,
        query: ExpandQuery,
    ) -> Result<Arc<T>, Self::Error> {
        let endpoint_url = self
            .redfish_endpoint
            .with_path_and_query(&id.to_string(), &query.to_query_string());

        self.get_with_cache(endpoint_url, id).await
    }

    async fn create<V: Sync + Send + Serialize, R: Sync + Send + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
        v: &V,
    ) -> Result<R, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        self.client.post(endpoint_url, v, &self.credentials).await
    }

    async fn update<V: Sync + Send + Serialize, R: Sync + Send + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
        etag: Option<&ODataETag>,
        v: &V,
    ) -> Result<R, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        let etag = etag
            .cloned()
            .unwrap_or_else(|| ODataETag::from(String::from("*")));
        self.client
            .patch(endpoint_url, etag, v, &self.credentials)
            .await
    }

    async fn delete(&self, id: &ODataId) -> Result<Empty, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        self.client.delete(endpoint_url, &self.credentials).await
    }

    async fn action<
        T: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'de> Deserialize<'de>,
    >(
        &self,
        action: &Action<T, R>,
        params: &T,
    ) -> Result<R, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&action.target.to_string());
        self.client
            .post(endpoint_url, params, &self.credentials)
            .await
    }

    async fn filter<T: EntityTypeRef + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
        query: FilterQuery,
    ) -> Result<Arc<T>, Self::Error> {
        let endpoint_url = self
            .redfish_endpoint
            .with_path_and_query(&id.to_string(), &query.to_query_string());

        self.get_with_cache(endpoint_url, id).await
    }
}

#[cfg(feature = "reqwest")]
#[derive(Debug)]
pub enum BmcReqwestError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error),
    InvalidResponse(Box<reqwest::Response>),
    CacheMiss,
    CacheError(String),
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for BmcReqwestError {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}

#[cfg(feature = "reqwest")]
impl CacheableError for BmcReqwestError {
    fn is_cached(&self) -> bool {
        match self {
            Self::InvalidResponse(response) => {
                response.status() == reqwest::StatusCode::NOT_MODIFIED
            }
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

#[cfg(feature = "reqwest")]
#[allow(clippy::absolute_paths)]
impl std::fmt::Display for BmcReqwestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReqwestError(e) => write!(f, "HTTP client error: {e}"),
            Self::InvalidResponse(response) => {
                write!(f, "Invalid HTTP response: {}", response.status())
            }
            Self::CacheMiss => write!(f, "Resource not found in cache"),
            Self::CacheError(r) => write!(f, "Error occurred in cache {r}"),
            Self::JsonError(e) => write!(f, "JSON conversion error error: {e}"),
        }
    }
}

#[cfg(feature = "reqwest")]
#[allow(clippy::absolute_paths)]
impl std::error::Error for BmcReqwestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReqwestError(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(feature = "reqwest")]
/// Configuration parameters for the reqwest HTTP client.
///
/// This struct allows customizing various aspects of the reqwest client behavior,
/// including timeouts, TLS settings, and connection pooling.
///
/// # Examples
///
/// ```rust
/// use nv_redfish_bmc_http::ReqwestClientParams;
/// use std::time::Duration;
///
/// let params = ReqwestClientParams::new()
///     .timeout(Duration::from_secs(30))
///     .connect_timeout(Duration::from_secs(10))
///     .user_agent("MyApp/1.0")
///     .accept_invalid_certs(true);
/// ```
#[derive(Debug, Clone)]
pub struct ReqwestClientParams {
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
}

impl Default for ReqwestClientParams {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            connect_timeout: Some(Duration::from_secs(10)),
            user_agent: Some("nv-redfish/0.1.0".to_string()),
            accept_invalid_certs: false,
            max_redirects: Some(10),
            tcp_keepalive: Some(Duration::from_secs(60)),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            pool_max_idle_per_host: Some(10),
        }
    }
}

#[cfg(feature = "reqwest")]
impl ReqwestClientParams {
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
    pub const fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }
}

#[cfg(feature = "reqwest")]
/// HTTP client implementation using the reqwest library.
///
/// This provides a concrete implementation of [`HttpClient`] using the popular
/// reqwest HTTP client library. It supports all standard HTTP features including
/// TLS, authentication, and connection pooling.
///
/// # Examples
///
/// ```rust,no_run
/// use nv_redfish_bmc_http::{ReqwestClient, HttpBmc};
/// use nv_redfish_bmc_http::BmcCredentials;
/// use nv_redfish_bmc_http::ReqwestClientParams;
/// use std::time::Duration;
/// use url::Url;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create with default settings
/// let client = ReqwestClient::new()?;
///
/// // Or with custom parameters
/// let params = ReqwestClientParams::new().timeout(Duration::from_secs(60));
/// let client = ReqwestClient::with_params(params)?;
///
/// // Use with HttpBmc
/// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
/// let endpoint = Url::parse("https://192.168.1.100")?;
/// let bmc = HttpBmc::new(client, endpoint, credentials);
/// # Ok(())
/// # }
/// ```
pub struct ReqwestClient {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest")]
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::absolute_paths)]
impl ReqwestClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_params(ReqwestClientParams::default())
    }

    pub fn with_params(params: ReqwestClientParams) -> Result<Self, reqwest::Error> {
        let mut builder = reqwest::Client::builder();

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

        Ok(Self {
            client: builder.build()?,
        })
    }

    #[must_use]
    pub const fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest")]
impl ReqwestClient {
    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T, BmcReqwestError>
    where
        T: DeserializeOwned,
    {
        if !response.status().is_success() {
            return Err(BmcReqwestError::InvalidResponse(Box::new(response)));
        }

        let etag_header = response.headers().get("etag").cloned();

        let mut value: serde_json::Value = response
            .json()
            .await
            .map_err(BmcReqwestError::ReqwestError)?;

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

        serde_json::from_value(value).map_err(BmcReqwestError::JsonError)
    }
}

#[cfg(feature = "reqwest")]
impl HttpClient for ReqwestClient {
    type Error = BmcReqwestError;

    async fn get<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        etag: Option<ODataETag>,
    ) -> Result<T, Self::Error>
    where
        T: DeserializeOwned,
    {
        let mut request = self
            .client
            .get(url)
            .basic_auth(&credentials.username, Some(credentials.password()));

        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag.to_string());
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    async fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
    ) -> Result<T, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let response = self
            .client
            .post(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
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
    ) -> Result<T, Self::Error>
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync,
    {
        let mut request = self
            .client
            .patch(url)
            .basic_auth(&credentials.username, Some(credentials.password()));

        request = request.header("If-Match", etag.to_string());

        let response = request.json(body).send().await?;
        self.handle_response(response).await
    }

    async fn delete(&self, url: Url, credentials: &BmcCredentials) -> Result<Empty, Self::Error> {
        let response = self
            .client
            .delete(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BmcReqwestError::InvalidResponse(Box::new(response)));
        }

        Ok(Empty {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "reqwest")]
    #[test]
    fn test_cacheable_error_trait() {
        let mock_response = reqwest::Response::from(
            http::Response::builder()
                .status(304)
                .body("")
                .expect("Valid empty body"),
        );
        let error = BmcReqwestError::InvalidResponse(Box::new(mock_response));
        assert!(error.is_cached());

        let cache_miss = BmcReqwestError::CacheMiss;
        assert!(!cache_miss.is_cached());

        let created_miss = BmcReqwestError::cache_miss();
        assert!(matches!(created_miss, BmcReqwestError::CacheMiss));
    }
}
