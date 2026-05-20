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

#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::unimplemented,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::duration_suboptimal_units)]
#![deny(missing_docs)]

//! HTTP implementation of [`nv_redfish_core::Bmc`] trait.

pub mod cache;
pub mod credentials;

#[cfg(feature = "reqwest")]
pub mod reqwest;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;
use std::sync::RwLock;

use crate::cache::TypeErasedCarCache;

use http::HeaderMap;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Action;
use nv_redfish_core::Bmc;
use nv_redfish_core::BoxTryStream;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::FilterQuery;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use nv_redfish_core::SessionCreateResponse;
use nv_redfish_core::UploadReader;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::Url;

#[doc(inline)]
pub use credentials::BmcCredentials;

#[doc(inline)]
pub use nv_redfish_core::MultipartUpdateRequest;

/// HTTP Client trait.
///
/// nv-redfish-bmc-http supports any HTTP implementation that
/// implements this [`HttpClient`] trait.
pub trait HttpClient: Send + Sync {
    /// HTTP client error.
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
    ) -> impl Future<Output = Result<ModificationResponse<T>, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Perform a Redfish session creation POST request.
    fn post_session<B, T>(
        &self,
        url: Url,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<SessionCreateResponse<T>, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Performs an UpdateService multipart upload with credentials and headers.
    ///
    /// The request carries `UpdateParameters`, `UpdateFile`, and optional OEM
    /// multipart parts.
    fn post_multipart_update<U, V, T>(
        &self,
        url: Url,
        request: MultipartUpdateRequest<'_, U, V>,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<ModificationResponse<T>, Self::Error>> + Send
    where
        U: UploadReader,
        T: DeserializeOwned + Send + Sync,
        V: Serialize + Send + Sync;

    /// Perform an HTTP PATCH request.
    fn patch<B, T>(
        &self,
        url: Url,
        etag: ODataETag,
        body: &B,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<ModificationResponse<T>, Self::Error>> + Send
    where
        B: Serialize + Send + Sync,
        T: DeserializeOwned + Send + Sync;

    /// Perform an HTTP DELETE request.
    fn delete<T>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<ModificationResponse<T>, Self::Error>> + Send
    where
        T: DeserializeOwned + Send + Sync;

    /// Open an SSE stream
    fn sse<T: Sized + for<'de> Deserialize<'de> + Send>(
        &self,
        url: Url,
        credentials: &BmcCredentials,
        custom_headers: &HeaderMap,
    ) -> impl Future<Output = Result<BoxTryStream<T, Self::Error>, Self::Error>> + Send;
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
pub struct HttpBmc<C: HttpClient> {
    client: C,
    redfish_endpoint: RedfishEndpoint,
    credentials: RwLock<Arc<BmcCredentials>>,
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
    /// let credentials = BmcCredentials::username_password("admin".to_string(), Some("password".to_string()));
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
    /// let credentials = BmcCredentials::username_password("admin".to_string(), Some("password".to_string()));
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
            credentials: RwLock::new(Arc::new(credentials)),
            cache: RwLock::new(TypeErasedCarCache::new(cache_settings.capacity)),
            etags: RwLock::new(HashMap::new()),
            custom_headers,
        }
    }

    /// Replace the credentials used for subsequent requests.
    ///
    /// Existing cache and ETag state is preserved.
    ///
    /// # Panics
    ///
    /// Panics if the internal credentials lock is poisoned. This should not
    /// occur in normal operation.
    #[allow(clippy::panic)] // See panics section.
    pub fn set_credentials(&self, credentials: BmcCredentials) {
        *self.credentials.write().expect("poisoned") = Arc::new(credentials);
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
#[derive(Clone, Copy)]
pub struct CacheSettings {
    capacity: usize,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self { capacity: 100 }
    }
}

impl CacheSettings {
    /// Define capacity of the cache measured in number of items.
    #[must_use]
    pub const fn with_capacity(capacity: usize) -> Self {
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
    #[allow(clippy::panic)] // See set_credentials Panic doc.
    fn read_credentials(&self) -> Arc<BmcCredentials> {
        self.credentials
            .read()
            .map(|credentials| Arc::clone(&credentials))
            .expect("lock poisoned")
    }

    /// Perform a GET request with `ETag` caching support
    ///
    /// This handles:
    /// - Retrieving cached `ETag` before request
    /// - Sending conditional GET with If-None-Match
    /// - Handling 304 Not Modified responses from cache
    /// - Updating cache and `ETag` storage on success
    #[allow(clippy::significant_drop_tightening)]
    async fn get_with_cache<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
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
        let credentials = self.read_credentials();

        // Perform GET request
        match self
            .client
            .get::<T>(
                endpoint_url,
                credentials.as_ref(),
                etag,
                &self.custom_headers,
            )
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

                    if let Some(evicted_id) = cache.put_typed(id.clone(), Arc::clone(&entity)) {
                        etags.remove(&evicted_id);
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

    async fn get<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        self.get_with_cache(endpoint_url, id).await
    }

    async fn expand<T: Expandable + 'static>(
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
    ) -> Result<ModificationResponse<R>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        let credentials = self.read_credentials();
        self.client
            .post(endpoint_url, v, credentials.as_ref(), &self.custom_headers)
            .await
    }

    async fn create_session<
        V: Sync + Send + Serialize,
        R: Sync + Send + for<'de> Deserialize<'de>,
    >(
        &self,
        id: &ODataId,
        v: &V,
    ) -> Result<SessionCreateResponse<R>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        let credentials = self.read_credentials();
        self.client
            .post_session(endpoint_url, v, credentials.as_ref(), &self.custom_headers)
            .await
    }

    async fn update<V: Sync + Send + Serialize, R: Sync + Send + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
        etag: Option<&ODataETag>,
        v: &V,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        let etag = etag
            .cloned()
            .unwrap_or_else(|| ODataETag::from(String::from("*")));
        let credentials = self.read_credentials();
        self.client
            .patch(
                endpoint_url,
                etag,
                v,
                credentials.as_ref(),
                &self.custom_headers,
            )
            .await
    }

    async fn delete<T: Sync + Send + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
    ) -> Result<ModificationResponse<T>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&id.to_string());
        let credentials = self.read_credentials();
        self.client
            .delete(endpoint_url, credentials.as_ref(), &self.custom_headers)
            .await
    }

    async fn action<T: Send + Sync + Serialize, R: Send + Sync + for<'de> Deserialize<'de>>(
        &self,
        action: &Action<T, R>,
        params: &T,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        let endpoint_url = self.redfish_endpoint.with_path(&action.target.to_string());
        let credentials = self.read_credentials();
        self.client
            .post(
                endpoint_url,
                params,
                credentials.as_ref(),
                &self.custom_headers,
            )
            .await
    }

    async fn multipart_update<U, V, R>(
        &self,
        uri: &str,
        request: MultipartUpdateRequest<'_, U, V>,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        U: UploadReader,
        R: Send + Sync + for<'de> Deserialize<'de>,
        V: Send + Sync + Serialize,
    {
        // MultipartHttpPushUri can be absolute or BMC-relative.
        // Match existing URI handling before adding auth and headers.
        let endpoint_url = Url::parse(uri).unwrap_or_else(|_| self.redfish_endpoint.with_path(uri));
        let credentials = self.read_credentials();

        self.client
            .post_multipart_update(
                endpoint_url,
                request,
                credentials.as_ref(),
                &self.custom_headers,
            )
            .await
    }

    async fn filter<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
        query: FilterQuery,
    ) -> Result<Arc<T>, Self::Error> {
        let endpoint_url = self
            .redfish_endpoint
            .with_path_and_query(&id.to_string(), &query.to_query_string());

        self.get_with_cache(endpoint_url, id).await
    }

    async fn stream<T: Send + Sized + for<'de> Deserialize<'de>>(
        &self,
        uri: &str,
    ) -> Result<BoxTryStream<T, Self::Error>, Self::Error> {
        let endpoint_url = Url::parse(uri).unwrap_or_else(|_| self.redfish_endpoint.with_path(uri));
        let credentials = self.read_credentials();
        self.client
            .sse(endpoint_url, credentials.as_ref(), &self.custom_headers)
            .await
    }
}
