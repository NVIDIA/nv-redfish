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

#[cfg(feature = "reqwest")]
pub mod reqwest;

use crate::cache::TypeErasedCarCache;
use http::HeaderMap;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Action;
use nv_redfish_core::Bmc;
use nv_redfish_core::Empty;
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
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP POST request.
    fn post<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
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
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP DELETE request.
    fn delete(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
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
/// use nv_redfish_bmc_http::HttpBmc;
/// use nv_redfish_bmc_http::CacheSettings;
/// use nv_redfish_bmc_http::BmcCredentials;
/// use nv_redfish_bmc_http::reqwest::Client;
/// use nv_redfish_core::{Bmc, ODataId};
/// use url::Url;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
/// let http_client = Client::new()?;
/// let endpoint = Url::parse("https://192.168.1.100")?;
///
/// let bmc = HttpBmc::new(http_client, endpoint, credentials, CacheSettings::default());
/// # Ok(())
/// # }
/// ```
pub struct HttpBmc<C: HttpClient> {
    client: C,
    redfish_endpoint: RedfishEndpoint,
    credentials: BmcCredentials,
    cache: RwLock<TypeErasedCarCache<ODataId>>,
    etags: RwLock<HashMap<ODataId, ODataETag>>,
    custom_headers: HeaderMap,
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
    /// use nv_redfish_bmc_http::HttpBmc;
    /// use nv_redfish_bmc_http::CacheSettings;
    /// use nv_redfish_bmc_http::BmcCredentials;
    /// use nv_redfish_bmc_http::reqwest::Client;
    /// use url::Url;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
    /// let http_client = Client::new()?;
    /// let endpoint = Url::parse("https://192.168.1.100")?;
    ///
    /// let bmc = HttpBmc::new(http_client, endpoint, credentials, CacheSettings::default());
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        client: C,
        redfish_endpoint: Url,
        credentials: BmcCredentials,
        cache_settings: CacheSettings,
    ) -> Self {
        Self::with_custom_headers(
            client,
            redfish_endpoint,
            credentials,
            cache_settings,
            HeaderMap::new(),
        )
    }

    /// Create a new HTTP-based BMC client with custom headers and ETag-based caching.
    ///
    /// This is an alternative constructor that allows specifying custom HTTP headers
    /// that will be included in all requests. Use this when you need vendor-specific
    /// headers, custom authentication tokens, or other HTTP headers required by the
    /// Redfish service at construction time.
    ///
    /// For most use cases, prefer [`HttpBmc::new`] which creates a client without
    /// custom headers.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client implementation to use for requests
    /// * `redfish_endpoint` - The base URL of the Redfish service (e.g., `https://192.168.1.100`)
    /// * `credentials` - Authentication credentials for the BMC
    /// * `cache_settings` - Cache configuration for response caching
    /// * `custom_headers` - Custom HTTP headers to include in all requests
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use nv_redfish_bmc_http::HttpBmc;
    /// use nv_redfish_bmc_http::CacheSettings;
    /// use nv_redfish_bmc_http::BmcCredentials;
    /// use nv_redfish_bmc_http::reqwest::Client;
    /// use url::Url;
    /// use http::HeaderMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let credentials = BmcCredentials::new("admin".to_string(), "password".to_string());
    /// let http_client = Client::new()?;
    /// let endpoint = Url::parse("https://192.168.1.100")?;
    ///
    /// // Create custom headers
    /// let mut headers = HeaderMap::new();
    /// headers.insert("X-Auth-Token", "custom-token-value".parse()?);
    /// headers.insert("X-Vendor-Header", "vendor-specific-value".parse()?);
    ///
    /// // Create BMC client with custom headers
    /// let bmc = HttpBmc::with_custom_headers(
    ///     http_client,
    ///     endpoint,
    ///     credentials,
    ///     CacheSettings::default(),
    ///     headers,
    /// );
    ///
    /// // All requests will include the custom headers
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_custom_headers(
        client: C,
        redfish_endpoint: Url,
        credentials: BmcCredentials,
        cache_settings: CacheSettings,
        custom_headers: HeaderMap,
    ) -> Self {
        Self {
            client,
            redfish_endpoint: RedfishEndpoint::from(redfish_endpoint),
            credentials,
            cache: RwLock::new(TypeErasedCarCache::new(cache_settings.capacity)),
            etags: RwLock::new(HashMap::new()),
            custom_headers,
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

/// `CacheSettings` for internal BMC cache with etags
pub struct CacheSettings {
    capacity: usize,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self { capacity: 100 }
    }
}

impl CacheSettings {
    pub fn with_capacity(capacity: usize) -> Self {
        Self { capacity }
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
            .get::<T>(endpoint_url, &self.credentials, etag, &self.custom_headers)
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
        self.client
            .post(endpoint_url, v, &self.credentials, &self.custom_headers)
            .await
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
            .patch(
                endpoint_url,
                etag,
                v,
                &self.credentials,
                &self.custom_headers,
            )
            .await
    }

    async fn delete(&self, id: &ODataId) -> Result<Empty, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        self.client
            .delete(endpoint_url, &self.credentials, &self.custom_headers)
            .await
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
            .post(
                endpoint_url,
                params,
                &self.credentials,
                &self.custom_headers,
            )
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
