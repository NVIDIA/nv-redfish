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

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};
use url::Url;

use crate::{Bmc, EntityType, Expandable, ODataETag, ODataId, bmc::BmcCredentials};

/// Builder for Redfish `$expand` query parameters according to DSP0266 specification.
///
/// The `$expand` query parameter allows clients to request that the server expand
/// navigation properties inline instead of returning just references. This is particularly
/// useful for reducing the number of HTTP requests needed to retrieve related data.
///
/// According to the [Redfish specification Table 9](https://redfish.dmtf.org/schemas/DSP0266_1.15.0.html#the-expand-query-parameter),
/// the supported expand options are:
///
/// | Option | Description | Example URL |
/// |--------|-------------|-------------|
/// | `*` | Expand all hyperlinks, including payload annotations | `?$expand=*` |
/// | `.` | Expand hyperlinks not in links property instances | `?$expand=.` |
/// | `~` | Expand hyperlinks in links property instances | `?$expand=~` |
/// | `$levels` | Number of levels to cascade expansion | `?$expand=.($levels=2)` |
///
/// # Examples
///
/// ```rust
/// use nv_redfish::http::ExpandQuery;
///
/// // Default: expand current resource one level
/// let default = ExpandQuery::default();
/// assert_eq!(default.to_query_string(), "$expand=.($levels=1)");
///
/// // Expand all hyperlinks
/// let all = ExpandQuery::all();
/// assert_eq!(all.to_query_string(), "$expand=*($levels=1)");
///
/// // Expand with multiple levels
/// let deep = ExpandQuery::current().levels(3);
/// assert_eq!(deep.to_query_string(), "$expand=.($levels=3)");
///
/// // Expand specific navigation property
/// let thermal = ExpandQuery::property("Thermal");
/// assert_eq!(thermal.to_query_string(), "$expand=Thermal($levels=1)");
/// ```
#[derive(Debug, Clone)]
pub struct ExpandQuery {
    /// The expand expression (*, ., ~, or specific navigation properties)
    expand_expression: String,
    /// Number of levels to cascade the expand operation (default is 1)
    levels: Option<u32>,
}

impl Default for ExpandQuery {
    /// Default expand query: $expand=.($levels=1)
    /// Expands all hyperlinks not in any links property instances of the resource
    fn default() -> Self {
        Self {
            expand_expression: ".".to_string(),
            levels: Some(1),
        }
    }
}

impl ExpandQuery {
    /// Create a new expand query with default values.
    ///
    /// This is equivalent to [`ExpandQuery::default()`] and creates a query
    /// that expands the current resource one level deep: `$expand=.($levels=1)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let query = ExpandQuery::new();
    /// assert_eq!(query.to_query_string(), "$expand=.($levels=1)");
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Expand all hyperlinks, including those in payload annotations.
    ///
    /// This expands all hyperlinks found in the resource, including those in payload
    /// annotations such as `@Redfish.Settings`, `@Redfish.ActionInfo`, and
    /// `@Redfish.CollectionCapabilities`.
    ///
    /// Equivalent to: `$expand=*`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let query = ExpandQuery::all();
    /// assert_eq!(query.to_query_string(), "$expand=*($levels=1)");
    ///
    /// // With multiple levels
    /// let deep = ExpandQuery::all().levels(3);
    /// assert_eq!(deep.to_query_string(), "$expand=*($levels=3)");
    /// ```
    pub fn all() -> Self {
        Self {
            expand_expression: "*".to_string(),
            levels: Some(1),
        }
    }

    /// Expand all hyperlinks not in any links property instances of the resource.
    ///
    /// This expands hyperlinks found directly in the resource properties, but not
    /// those in dedicated `Links` sections. Includes payload annotations.
    ///
    /// Equivalent to: `$expand=.`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let query = ExpandQuery::current();
    /// assert_eq!(query.to_query_string(), "$expand=.($levels=1)");
    /// ```
    pub fn current() -> Self {
        Self {
            expand_expression: ".".to_string(),
            levels: Some(1),
        }
    }

    /// Expand all hyperlinks found in all links property instances of the resource.
    ///
    /// This expands only hyperlinks found in `Links` sections of the resource,
    /// which typically contain references to related resources.
    ///
    /// Equivalent to: `$expand=~`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let query = ExpandQuery::links();
    /// assert_eq!(query.to_query_string(), "$expand=~($levels=1)");
    /// ```
    pub fn links() -> Self {
        Self {
            expand_expression: "~".to_string(),
            levels: Some(1),
        }
    }

    /// Expand a specific navigation property.
    ///
    /// This expands only the specified navigation property, which is useful when you
    /// know exactly which related data you need.
    ///
    /// # Arguments
    ///
    /// * `property` - The name of the navigation property to expand
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let thermal = ExpandQuery::property("Thermal");
    /// assert_eq!(thermal.to_query_string(), "$expand=Thermal($levels=1)");
    ///
    /// let members = ExpandQuery::property("Members");
    /// assert_eq!(members.to_query_string(), "$expand=Members($levels=1)");
    /// ```
    pub fn property<S: Into<String>>(property: S) -> Self {
        Self {
            expand_expression: property.into(),
            levels: Some(1),
        }
    }

    /// Expand multiple specific navigation properties.
    ///
    /// This allows expanding several navigation properties in a single request,
    /// which is more efficient than making separate requests for each property.
    ///
    /// # Arguments
    ///
    /// * `properties` - A slice of navigation property names to expand
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let env = ExpandQuery::properties(&["Thermal", "Power"]);
    /// assert_eq!(env.to_query_string(), "$expand=Thermal,Power($levels=1)");
    ///
    /// let system = ExpandQuery::properties(&["Processors", "Memory", "Storage"]);
    /// assert_eq!(system.to_query_string(), "$expand=Processors,Memory,Storage($levels=1)");
    /// ```
    pub fn properties(properties: &[&str]) -> Self {
        Self {
            expand_expression: properties.join(","),
            levels: Some(1),
        }
    }

    /// Set the number of levels to cascade the expand operation.
    ///
    /// The `$levels` parameter controls how deep the expansion goes:
    /// - Level 1: Expand hyperlinks in the current resource
    /// - Level 2: Also expand hyperlinks in the resources expanded at level 1
    /// - And so on...
    ///
    /// # Arguments
    ///
    /// * `levels` - Number of levels to expand (typically 1-6 in practice)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let shallow = ExpandQuery::current().levels(1);
    /// assert_eq!(shallow.to_query_string(), "$expand=.($levels=1)");
    ///
    /// let deep = ExpandQuery::all().levels(3);
    /// assert_eq!(deep.to_query_string(), "$expand=*($levels=3)");
    /// ```
    pub fn levels(mut self, levels: u32) -> Self {
        self.levels = Some(levels);
        self
    }

    /// Convert to the OData query string according to Redfish specification.
    ///
    /// This generates the actual query parameter string that will be appended to
    /// HTTP requests to Redfish services.
    ///
    /// # Returns
    ///
    /// A query string in the format `$expand=expression($levels=n)` or just
    /// `$expand=expression` if no levels are specified.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nv_redfish::http::ExpandQuery;
    ///
    /// let query = ExpandQuery::property("Thermal").levels(2);
    /// assert_eq!(query.to_query_string(), "$expand=Thermal($levels=2)");
    ///
    /// let query = ExpandQuery::all();
    /// assert_eq!(query.to_query_string(), "$expand=*($levels=1)");
    /// ```
    pub fn to_query_string(&self) -> String {
        match self.levels {
            Some(levels) => format!("$expand={}($levels={})", self.expand_expression, levels),
            None => format!("$expand={}", self.expand_expression),
        }
    }
}

#[cfg(feature = "reqwest")]
use std::time::Duration;

pub trait HttpClient: Send + Sync {
    type Error: Send;

    /// Perform an HTTP GET request with optional conditional headers.
    fn get<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        etag: Option<ODataETag>,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        T: DeserializeOwned;

    /// Perform an HTTP POST request.
    fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        B: Serialize + Sync,
        T: DeserializeOwned + Send;
}

/// ETag-based cache for Redfish resources
///
/// This cache stores resources along with their ETags and uses HTTP conditional
/// requests with If-None-Match headers to validate cache entries.
#[derive(Debug)]
struct ResourceCache {
    /// Cache storage mapping resource IDs to their cached data  
    entries: RwLock<HashMap<ODataId, Box<dyn std::any::Any + Send + Sync>>>,
    /// ETag storage for conditional requests
    etags: RwLock<HashMap<ODataId, ODataETag>>,
}

impl ResourceCache {
    fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            etags: RwLock::new(HashMap::new()),
        }
    }

    /// Get cached resource - always returns the cached version if present
    /// Validation should be done via conditional HTTP request
    fn get<T: EntityType + 'static>(&self, id: &ODataId) -> Option<Arc<T>> {
        let entries = self.entries.read().ok()?;
        if let Some(entry_any) = entries.get(id) {
            if let Some(resource) = entry_any.downcast_ref::<Arc<T>>() {
                return Some(Arc::clone(resource));
            }
        }
        None
    }

    /// Store resource in cache with ETag
    fn put<T: EntityType + 'static + Send + Sync>(
        &self,
        id: &ODataId,
        resource: Arc<T>,
        etag: Option<ODataETag>,
    ) {
        if let Some(etag_value) = etag {
            if let Ok(mut etags) = self.etags.write() {
                etags.insert(id.clone(), etag_value);
            }

            if let Ok(mut entries) = self.entries.write() {
                entries.insert(id.clone(), Box::new(resource));
            }
        }
    }

    /// Get stored ETag for conditional request (If-None-Match header)
    fn get_etag(&self, id: &ODataId) -> Option<ODataETag> {
        self.etags.read().ok()?.get(id).cloned()
    }

    /// Invalidate cache entry - removes both resource and ETag
    fn invalidate(&self, id: &ODataId) {
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(id);
        }
        if let Ok(mut etags) = self.etags.write() {
            etags.remove(id);
        }
    }

    /// Clear all cache entries
    fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
        if let Ok(mut etags) = self.etags.write() {
            etags.clear();
        }
    }
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
/// use nv_redfish::http::{HttpBmc, ReqwestClient};
/// use nv_redfish::bmc::BmcCredentials;
/// use nv_redfish::{Bmc, ODataId};
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
    redfish_endpoint: Url,
    credentials: BmcCredentials,
    cache: ResourceCache,
}

impl<C: HttpClient> HttpBmc<C> {
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
    /// use nv_redfish::http::{HttpBmc, ReqwestClient};
    /// use nv_redfish::bmc::BmcCredentials;
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
            redfish_endpoint,
            credentials,
            cache: ResourceCache::new(),
        }
    }

    /// Clear all cached resources
    ///
    /// This forces all subsequent requests to fetch fresh data from the BMC.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Invalidate a specific cached resource
    ///
    /// This forces the next request for this resource to fetch fresh data.
    pub fn invalidate_cache(&self, id: &ODataId) {
        self.cache.invalidate(id);
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
}

impl<C: HttpClient> Bmc for HttpBmc<C>
where
    C::Error: CacheableError,
{
    type Error = C::Error;

    async fn get<T: EntityType + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        let mut endpoint_url = self.redfish_endpoint.clone();
        endpoint_url.set_path(&id.to_string());

        let etag = self.cache.get_etag(id);

        match self
            .client
            .get::<T>(endpoint_url, &self.credentials, etag)
            .await
        {
            Ok(response) => {
                let entity = Arc::new(response);
                self.cache
                    .put(id, Arc::clone(&entity), entity.etag().clone());
                Ok(entity)
            }
            Err(e) => {
                if e.is_cached() {
                    self.cache.get::<T>(id).ok_or_else(C::Error::cache_miss)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn expand<T: Expandable>(
        &self,
        id: &ODataId,
        query: ExpandQuery,
    ) -> Result<Arc<T>, Self::Error> {
        let mut endpoint_url = self.redfish_endpoint.clone();
        endpoint_url.set_path(&id.to_string());
        endpoint_url.set_query(Some(&query.to_query_string()));

        self.client
            .get::<T>(endpoint_url, &self.credentials, None)
            .await
            .map(Arc::new)
    }
}

#[cfg(feature = "reqwest")]
#[derive(Debug)]
pub enum BmcReqwestError {
    ReqwestError(reqwest::Error),
    InvalidResponse(Box<reqwest::Response>),
    CacheMiss,
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for BmcReqwestError {
    fn from(value: reqwest::Error) -> Self {
        BmcReqwestError::ReqwestError(value)
    }
}

#[cfg(feature = "reqwest")]
impl CacheableError for BmcReqwestError {
    fn is_cached(&self) -> bool {
        match self {
            BmcReqwestError::InvalidResponse(response) => {
                response.status() == reqwest::StatusCode::NOT_MODIFIED
            }
            _ => false,
        }
    }

    fn cache_miss() -> Self {
        BmcReqwestError::CacheMiss
    }
}

#[cfg(feature = "reqwest")]
impl std::fmt::Display for BmcReqwestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BmcReqwestError::ReqwestError(e) => write!(f, "HTTP client error: {e}"),
            BmcReqwestError::InvalidResponse(response) => {
                write!(f, "Invalid HTTP response: {}", response.status())
            }
            BmcReqwestError::CacheMiss => {
                write!(f, "Resource not found in cache")
            }
        }
    }
}

#[cfg(feature = "reqwest")]
impl std::error::Error for BmcReqwestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BmcReqwestError::ReqwestError(e) => Some(e),
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
/// use nv_redfish::http::ReqwestClientParams;
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    pub fn max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = Some(max);
        self
    }

    pub fn tcp_keepalive(mut self, keepalive: Duration) -> Self {
        self.tcp_keepalive = Some(keepalive);
        self
    }

    pub fn no_timeout(mut self) -> Self {
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
/// use nv_redfish::http::{ReqwestClient, HttpBmc};
/// use nv_redfish::bmc::BmcCredentials;
/// use nv_redfish::http::ReqwestClientParams;
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

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
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
        let status = response.status();

        if !status.is_success() {
            return Err(BmcReqwestError::InvalidResponse(Box::new(response)));
        }

        let data = response
            .json()
            .await
            .map_err(BmcReqwestError::ReqwestError)?;

        Ok(data)
    }

    async fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
    ) -> Result<T, Self::Error>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let response = self
            .client
            .post(url)
            .basic_auth(&credentials.username, Some(credentials.password()))
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BmcReqwestError::InvalidResponse(Box::new(response)));
        }

        response.json().await.map_err(BmcReqwestError::ReqwestError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_expand() {
        let query = ExpandQuery::default();
        assert_eq!(query.to_query_string(), "$expand=.($levels=1)");
    }

    #[test]
    fn test_expand_all() {
        let query = ExpandQuery::all();
        assert_eq!(query.to_query_string(), "$expand=*($levels=1)");
    }

    #[test]
    fn test_expand_current() {
        let query = ExpandQuery::current();
        assert_eq!(query.to_query_string(), "$expand=.($levels=1)");
    }

    #[test]
    fn test_expand_links() {
        let query = ExpandQuery::links();
        assert_eq!(query.to_query_string(), "$expand=~($levels=1)");
    }

    #[test]
    fn test_expand_property() {
        let query = ExpandQuery::property("Thermal");
        assert_eq!(query.to_query_string(), "$expand=Thermal($levels=1)");
    }

    #[test]
    fn test_expand_properties() {
        let query = ExpandQuery::properties(&["Thermal", "Power"]);
        assert_eq!(query.to_query_string(), "$expand=Thermal,Power($levels=1)");
    }

    #[test]
    fn test_expand_with_levels() {
        let query = ExpandQuery::all().levels(3);
        assert_eq!(query.to_query_string(), "$expand=*($levels=3)");
    }

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_cacheable_error_trait() {
        let mock_response =
            reqwest::Response::from(http::Response::builder().status(304).body("").unwrap());
        let error = BmcReqwestError::InvalidResponse(Box::new(mock_response));
        assert!(error.is_cached());

        let cache_miss = BmcReqwestError::CacheMiss;
        assert!(!cache_miss.is_cached());

        let created_miss = BmcReqwestError::cache_miss();
        assert!(matches!(created_miss, BmcReqwestError::CacheMiss));
    }
}
