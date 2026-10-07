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

//! The compatibility layer: a [`Bmc`] over another that applies the
//! classified platform's document repairs to everything it reads.
//!
//! [`CompatBmc`] fetches each document raw, applies the repairs enabled for
//! the platform and then the caller's [`UserRules`] to every document in it
//! — members a device expanded inline included, at any depth — and only
//! then deserializes into the type the caller asked for. The entity a
//! create, update or delete answers with is repaired the same way. Actions,
//! uploads and event streams are delegated unchanged. It is classified once, from the service
//! root, with one request.
//!
//! The transport underneath caches the raw document, not the repaired one,
//! so a replaced rule set applies to the next read of a resource whose
//! `ETag` has not changed. The raw document carries the device's
//! `@odata.etag`, so a caching transport revalidates reads through the
//! layer exactly as it does typed ones.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use futures_util::TryStreamExt as _;
use nv_redfish_core::query::ExpandQuery;
use nv_redfish_core::Action;
use nv_redfish_core::ActionError;
use nv_redfish_core::Bmc;
use nv_redfish_core::BoxTryStream;
use nv_redfish_core::EntityTypeRef;
use nv_redfish_core::Expandable;
use nv_redfish_core::FilterQuery;
#[cfg(feature = "update-service-deprecated")]
use nv_redfish_core::HttpPushUriUpdateRequest;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::MultipartUpdateRequest;
use nv_redfish_core::NavProperty;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use nv_redfish_core::SessionCreateResponse;
use nv_redfish_core::StreamEvent;
use nv_redfish_core::UploadReader;
use serde::Deserialize;
use serde::Serialize;

use crate::rules;
use crate::BmcQuirks;
use crate::Raw;
use crate::RootEvidence;
use crate::UserRules;

/// A repaired document that still did not deserialize, with the path to
/// the property that failed.
pub type DecodeError = serde_path_to_error::Error<serde_json::Error>;

/// What a read through [`CompatBmc`] can fail with.
#[derive(Debug)]
pub enum CompatError<E> {
    /// The transport underneath failed.
    Transport(E),
    /// The document, repaired, still did not deserialize into the type
    /// asked for.
    Decode(DecodeError),
}

impl<E: fmt::Display> fmt::Display for CompatError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "BMC error: {error}"),
            Self::Decode(error) => write!(f, "repaired document did not deserialize: {error}"),
        }
    }
}

impl<E: StdError + 'static> StdError for CompatError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Decode(error) => Some(error),
        }
    }
}

impl<E: ActionError> ActionError for CompatError<E> {
    fn not_supported() -> Self {
        Self::Transport(E::not_supported())
    }
}

/// A [`Bmc`] that repairs what it reads.
///
/// Repairs are the classified platform's, then the caller's rules;
/// everything else is delegated to the transport underneath. Clones share
/// the caller's rule set.
pub struct CompatBmc<B: Bmc> {
    inner: Arc<B>,
    quirks: Arc<BmcQuirks>,
    user: UserRules,
}

impl<B: Bmc> CompatBmc<B> {
    /// Classifies the endpoint from its service root, one request, and
    /// returns the layer over `bmc`.
    ///
    /// # Errors
    ///
    /// The service root could not be read.
    pub async fn classify(bmc: Arc<B>) -> Result<Self, CompatError<B::Error>> {
        let root = NavProperty::<Raw>::new_reference(ODataId::service_root())
            .get(bmc.as_ref())
            .await
            .map_err(CompatError::Transport)?;
        let quirks = BmcQuirks::classify(&RootEvidence::from_root(root.value()));
        Ok(Self::new(bmc, Arc::new(quirks)))
    }

    /// The layer over `inner` for an already classified platform.
    #[must_use]
    pub fn new(inner: Arc<B>, quirks: Arc<BmcQuirks>) -> Self {
        Self {
            inner,
            quirks,
            user: UserRules::default(),
        }
    }

    /// The transport underneath, for a request that must see the device's
    /// document as it came.
    #[must_use]
    pub const fn inner(&self) -> &Arc<B> {
        &self.inner
    }

    /// The same layer, rules shared, over another transport for the same
    /// endpoint.
    #[must_use]
    pub fn replace_inner(&self, inner: Arc<B>) -> Self {
        Self {
            inner,
            quirks: Arc::clone(&self.quirks),
            user: self.user.clone(),
        }
    }

    /// The platform this layer repairs for.
    #[must_use]
    pub const fn quirks(&self) -> &Arc<BmcQuirks> {
        &self.quirks
    }

    /// The caller's rules, applied after the platform's. Replacing them
    /// through this handle affects every clone of the layer.
    #[must_use]
    pub const fn user_rules(&self) -> &UserRules {
        &self.user
    }

    fn repaired<T>(&self, raw: Result<Arc<Raw>, B::Error>) -> Result<Arc<T>, CompatError<B::Error>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let raw = raw.map_err(CompatError::Transport)?;
        // A transport that does not cache hands over the only reference.
        let document = Arc::try_unwrap(raw).map_or_else(|raw| raw.value().clone(), Raw::into_value);
        self.decode(document)
            .map(Arc::new)
            .map_err(CompatError::Decode)
    }

    /// Repairs a document obtained some other way and deserializes it into
    /// `T`, as a read through the layer would.
    ///
    /// # Errors
    ///
    /// The repaired document does not deserialize into `T`.
    pub fn decode<T>(&self, mut document: serde_json::Value) -> Result<T, DecodeError>
    where
        T: for<'de> Deserialize<'de>,
    {
        rules::repair_in_place(&self.quirks, &self.user.snapshot(), &mut document);
        serde_path_to_error::deserialize(document)
    }

    /// The entity a modification answered with, repaired.
    fn repaired_entity<R>(
        &self,
        response: Result<ModificationResponse<Raw>, B::Error>,
    ) -> Result<ModificationResponse<R>, CompatError<B::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        response
            .map_err(CompatError::Transport)?
            .try_map_entity(|raw| self.decode(raw.into_value()).map_err(CompatError::Decode))
    }
}

// Cloning shares the transport and the classification; `B` need not be
// `Clone`.
impl<B: Bmc> Clone for CompatBmc<B> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            quirks: Arc::clone(&self.quirks),
            user: self.user.clone(),
        }
    }
}

impl<B: Bmc> Bmc for CompatBmc<B> {
    type Error = CompatError<B::Error>;

    async fn expand<T: Expandable>(
        &self,
        id: &ODataId,
        query: ExpandQuery,
    ) -> Result<Arc<T>, Self::Error> {
        self.repaired::<T>(self.inner.expand::<Raw>(id, query).await)
    }

    async fn get<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        self.repaired::<T>(self.inner.get::<Raw>(id).await)
    }

    async fn filter<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
        query: FilterQuery,
    ) -> Result<Arc<T>, Self::Error> {
        self.repaired::<T>(self.inner.filter::<Raw>(id, query).await)
    }

    async fn create<V: Send + Sync + Serialize, R: Send + Sync + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
        query: &V,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        self.repaired_entity(self.inner.create::<V, Raw>(id, query).await)
    }

    async fn create_session<
        V: Send + Sync + Serialize,
        R: Send + Sync + for<'de> Deserialize<'de>,
    >(
        &self,
        id: &ODataId,
        query: &V,
    ) -> Result<SessionCreateResponse<R>, Self::Error> {
        let response = self
            .inner
            .create_session::<V, Raw>(id, query)
            .await
            .map_err(CompatError::Transport)?;
        Ok(SessionCreateResponse {
            entity: self
                .decode(response.entity.into_value())
                .map_err(CompatError::Decode)?,
            auth_token: response.auth_token,
            location: response.location,
        })
    }

    async fn update<
        V: Sync + Send + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    >(
        &self,
        id: &ODataId,
        etag: Option<&ODataETag>,
        update: &V,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        self.repaired_entity(self.inner.update::<V, Raw>(id, etag, update).await)
    }

    async fn delete<R: EntityTypeRef + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        self.repaired_entity(self.inner.delete::<Raw>(id).await)
    }

    async fn action<
        T: Send + Sync + Serialize,
        R: Send + Sync + Sized + for<'de> Deserialize<'de>,
    >(
        &self,
        action: &Action<T, R>,
        params: &T,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        self.inner
            .action(action, params)
            .await
            .map_err(CompatError::Transport)
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
        self.inner
            .multipart_update(uri, request)
            .await
            .map_err(CompatError::Transport)
    }

    #[cfg(feature = "update-service-deprecated")]
    async fn http_push_uri_update<U, R>(
        &self,
        uri: &str,
        request: HttpPushUriUpdateRequest<U>,
    ) -> Result<ModificationResponse<R>, Self::Error>
    where
        U: UploadReader,
        R: Send + Sync + for<'de> Deserialize<'de>,
    {
        self.inner
            .http_push_uri_update(uri, request)
            .await
            .map_err(CompatError::Transport)
    }

    async fn stream<T: Sized + for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        uri: &str,
    ) -> Result<BoxTryStream<T, Self::Error>, Self::Error> {
        let stream = self
            .inner
            .stream::<T>(uri)
            .await
            .map_err(CompatError::Transport)?;
        let mapped: BoxTryStream<T, Self::Error> = Box::pin(stream.map_err(CompatError::Transport));
        Ok(mapped)
    }

    async fn stream_events<T: Sized + for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        uri: &str,
        last_event_id: Option<&str>,
    ) -> Result<BoxTryStream<StreamEvent<T>, Self::Error>, Self::Error> {
        let stream = self
            .inner
            .stream_events::<T>(uri, last_event_id)
            .await
            .map_err(CompatError::Transport)?;
        let mapped: BoxTryStream<StreamEvent<T>, Self::Error> =
            Box::pin(stream.map_err(CompatError::Transport));
        Ok(mapped)
    }
}
