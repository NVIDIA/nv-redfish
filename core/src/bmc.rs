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

//! Baseboard Management Controller (BMC) client abstraction
//!
//! This module defines the transport-agnostic [`Bmc`] trait — a minimal
//! interface for interacting with Redfish services. Implementors provide
//! asynchronous operations to retrieve and expand entities, create/update
//! resources, delete entities, and invoke actions.
//!
//! Key concepts:
//! - Entity identity: Every entity is identified by an `@odata.id` ([`crate::ODataId`]).
//! - Entity reference: Generated types implement [`crate::EntityTypeRef`], which
//!   exposes `id()` and optional `etag()` accessors.
//! - Arc-based sharing: Read operations return `Arc<T>` to enable cheap sharing
//!   and caching while keeping values immutable.
//! - Expansion: [`crate::Expandable`] entities can request inline expansion using
//!   [`crate::query::ExpandQuery`], matching Redfish DSP0266 semantics for `$expand`.
//! - Actions: Actions are described by [`crate::Action<T, R>`] and are invoked via
//!   the `action` method.
//!
//! Operation semantics:
//! - `get` fetches the entity at the given `@odata.id`.
//! - `expand` fetches the entity with the provided `$expand` query.
//! - `create` typically performs a POST to a collection identified by `id` and
//!   returns the server-provided representation (`R`).
//! - `update` typically performs a PATCH on an entity identified by `id` and
//!   returns the updated representation (`R`).
//! - `delete` removes the entity at `id`.
//! - `action` posts to an action endpoint (`Action.target`).
//!
//! Notes for implementors:
//! - The trait is `Send + Sync` and returns `Send` futures to support use in
//!   async runtimes and multithreaded contexts.
//! - Implementations may include client-side caching or conditional requests;
//!   these details are intentionally abstracted behind the trait.
//! - Errors should implement `std::error::Error` and be safely transferable
//!   across threads.

use serde::Deserialize;
use serde::Serialize;

use crate::query::ExpandQuery;
use crate::Action;
use crate::Empty;
use crate::EntityTypeRef;
use crate::Expandable;
use crate::FilterQuery;
use crate::ODataETag;
use crate::ODataId;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;

/// BMC trait defines access to a Baseboard Management Controller using
/// the Redfish protocol.
pub trait Bmc: Send + Sync {
    /// BMC Error.
    type Error: StdError + Send + Sync;

    /// Expand any expandable object (navigation property or entity).
    ///
    /// `T` is structure that is used for return type.
    fn expand<T: Expandable + Send + Sync + 'static>(
        &self,
        id: &ODataId,
        query: ExpandQuery,
    ) -> impl Future<Output = Result<Arc<T>, Self::Error>> + Send;

    /// Get data of the object (navigation property or entity).
    ///
    /// `T` is structure that is used for return type.
    fn get<T: EntityTypeRef + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
    ) -> impl Future<Output = Result<Arc<T>, Self::Error>> + Send;

    /// Get and filters data of the object (navigation property or entity).
    ///
    /// `T` is structure that is used for return type.
    fn filter<T: EntityTypeRef + Sized + for<'a> Deserialize<'a> + 'static + Send + Sync>(
        &self,
        id: &ODataId,
        query: FilterQuery,
    ) -> impl Future<Output = Result<Arc<T>, Self::Error>> + Send;

    /// Creates element of the collection.
    ///
    /// `V` is structure that is used for create.
    /// `R` is structure that is used for return type.
    fn create<V: Sync + Send + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        id: &ODataId,
        query: &V,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;

    /// Update entity.
    ///
    /// `V` is structure that is used for update.
    /// `R` is structure that is used for return type (updated entity).
    fn update<V: Sync + Send + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        id: &ODataId,
        etag: Option<&ODataETag>,
        query: &V,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;

    /// Delete entity.
    fn delete(&self, id: &ODataId) -> impl Future<Output = Result<Empty, Self::Error>> + Send;

    /// Run action.
    ///
    /// `T` is structure that contains action parameters.
    /// `R` is structure with return type.
    fn action<T: Send + Sync + Serialize, R: Send + Sync + Sized + for<'a> Deserialize<'a>>(
        &self,
        action: &Action<T, R>,
        params: &T,
    ) -> impl Future<Output = Result<R, Self::Error>> + Send;
}
