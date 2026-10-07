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
//! uploads and event streams are delegated unchanged. It is classified
//! once, from the service root, with one request.
//!
//! The repair pass costs a second decode, so the layer only pays it when
//! something can apply: on a platform with no repairs and with no rules
//! from the caller, every request goes to the transport as it is.
//!
//! The transport underneath caches the raw document, not the repaired one,
//! so a replaced rule set applies to the next read of a resource whose
//! `ETag` has not changed. The layer remembers what it decoded from a
//! document the transport kept, under the rule set's generation, so a
//! revalidated read under the same rules is neither repaired nor decoded
//! again. The raw document carries the device's `@odata.etag`, so a caching
//! transport revalidates reads through the layer exactly as it does typed
//! ones.

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

use crate::memo::Memo;
use crate::rules;
use crate::user::Generation;
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
    /// Whether the platform enables any repair.
    platform_repairs: bool,
    user: UserRules,
    memo: Arc<Memo>,
}

/// One read's repairs: the platform's, then the caller's rules as they
/// stood when the read began. The platform's are fixed for the layer, so
/// the rule set's generation identifies them all.
struct Repairs {
    user: Generation,
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
            platform_repairs: rules::any_enabled(&quirks),
            quirks,
            user: UserRules::default(),
            memo: Arc::default(),
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
            platform_repairs: self.platform_repairs,
            user: self.user.clone(),
            memo: Arc::default(),
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

    /// The repairs a read needs now; `None` when neither the platform nor
    /// the caller has any, and the read goes to the transport as it is.
    fn repairs(&self) -> Option<Repairs> {
        let user = self.user.current();
        (self.platform_repairs || !user.rules.is_empty()).then_some(Repairs { user })
    }

    fn repair_and_decode<T>(
        &self,
        repairs: &Repairs,
        mut document: serde_json::Value,
    ) -> Result<T, DecodeError>
    where
        T: for<'de> Deserialize<'de>,
    {
        rules::repair_in_place(&self.quirks, &repairs.user.rules, &mut document);
        serde_path_to_error::deserialize(document)
    }

    /// The typed result of a raw read, repaired.
    fn repaired<T>(
        &self,
        repairs: &Repairs,
        raw: Result<Arc<Raw>, B::Error>,
    ) -> Result<Arc<T>, CompatError<B::Error>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        let raw = raw.map_err(CompatError::Transport)?;
        let generation = repairs.user.id;
        match Arc::try_unwrap(raw) {
            // The transport kept no reference: nothing to remember.
            Ok(raw) => self
                .repair_and_decode(repairs, raw.into_value())
                .map(Arc::new)
                .map_err(CompatError::Decode),
            // The transport cached the document; it answers a revalidated
            // read with this same `Arc`.
            Err(raw) => {
                if let Some(typed) = self.memo.get::<T>(&raw, generation) {
                    return Ok(typed);
                }
                let typed = self
                    .repair_and_decode(repairs, raw.value().clone())
                    .map(Arc::new)
                    .map_err(CompatError::Decode)?;
                self.memo.put(&raw, generation, Arc::clone(&typed));
                Ok(typed)
            }
        }
    }

    /// Repairs a document obtained some other way and deserializes it into
    /// `T`, as a read through the layer would.
    ///
    /// # Errors
    ///
    /// The repaired document does not deserialize into `T`.
    pub fn decode<T>(&self, document: serde_json::Value) -> Result<T, DecodeError>
    where
        T: for<'de> Deserialize<'de>,
    {
        match self.repairs() {
            Some(repairs) => self.repair_and_decode(&repairs, document),
            None => serde_path_to_error::deserialize(document),
        }
    }

    /// The entity a modification answered with, repaired.
    fn repaired_entity<R>(
        &self,
        repairs: &Repairs,
        response: Result<ModificationResponse<Raw>, B::Error>,
    ) -> Result<ModificationResponse<R>, CompatError<B::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        response
            .map_err(CompatError::Transport)?
            .try_map_entity(|raw| {
                self.repair_and_decode(repairs, raw.into_value())
                    .map_err(CompatError::Decode)
            })
    }
}

// Cloning shares the transport and the classification; `B` need not be
// `Clone`.
impl<B: Bmc> Clone for CompatBmc<B> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            quirks: Arc::clone(&self.quirks),
            platform_repairs: self.platform_repairs,
            user: self.user.clone(),
            memo: Arc::clone(&self.memo),
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
        match self.repairs() {
            Some(repairs) => self.repaired(&repairs, self.inner.expand::<Raw>(id, query).await),
            None => self
                .inner
                .expand(id, query)
                .await
                .map_err(CompatError::Transport),
        }
    }

    async fn get<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        match self.repairs() {
            Some(repairs) => self.repaired(&repairs, self.inner.get::<Raw>(id).await),
            None => self.inner.get(id).await.map_err(CompatError::Transport),
        }
    }

    async fn filter<T: EntityTypeRef + for<'de> Deserialize<'de> + 'static>(
        &self,
        id: &ODataId,
        query: FilterQuery,
    ) -> Result<Arc<T>, Self::Error> {
        match self.repairs() {
            Some(repairs) => self.repaired(&repairs, self.inner.filter::<Raw>(id, query).await),
            None => self
                .inner
                .filter(id, query)
                .await
                .map_err(CompatError::Transport),
        }
    }

    async fn create<V: Send + Sync + Serialize, R: Send + Sync + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
        query: &V,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        match self.repairs() {
            Some(repairs) => {
                self.repaired_entity(&repairs, self.inner.create::<V, Raw>(id, query).await)
            }
            None => self
                .inner
                .create(id, query)
                .await
                .map_err(CompatError::Transport),
        }
    }

    async fn create_session<
        V: Send + Sync + Serialize,
        R: Send + Sync + for<'de> Deserialize<'de>,
    >(
        &self,
        id: &ODataId,
        query: &V,
    ) -> Result<SessionCreateResponse<R>, Self::Error> {
        let Some(repairs) = self.repairs() else {
            return self
                .inner
                .create_session(id, query)
                .await
                .map_err(CompatError::Transport);
        };
        let response = self
            .inner
            .create_session::<V, Raw>(id, query)
            .await
            .map_err(CompatError::Transport)?;
        Ok(SessionCreateResponse {
            entity: self
                .repair_and_decode(&repairs, response.entity.into_value())
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
        match self.repairs() {
            Some(repairs) => self.repaired_entity(
                &repairs,
                self.inner.update::<V, Raw>(id, etag, update).await,
            ),
            None => self
                .inner
                .update(id, etag, update)
                .await
                .map_err(CompatError::Transport),
        }
    }

    async fn delete<R: EntityTypeRef + for<'de> Deserialize<'de>>(
        &self,
        id: &ODataId,
    ) -> Result<ModificationResponse<R>, Self::Error> {
        match self.repairs() {
            Some(repairs) => self.repaired_entity(&repairs, self.inner.delete::<Raw>(id).await),
            None => self.inner.delete(id).await.map_err(CompatError::Transport),
        }
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
