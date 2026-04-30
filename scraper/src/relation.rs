// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::TypeId;

use nv_redfish_core::ODataId;

/// Typed reference to a stored Redfish resource.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ResourceRef {
    /// Rust type id for the referenced resource.
    pub type_id: TypeId,
    /// Resource `@odata.id`.
    pub id: ODataId,
}

impl ResourceRef {
    /// Creates a typed resource reference.
    #[must_use]
    pub fn new(type_id: TypeId, id: impl Into<ODataId>) -> Self {
        Self {
            type_id,
            id: id.into(),
        }
    }

    /// Creates a resource reference for resource type `T`.
    #[must_use]
    pub fn of<T>(id: impl Into<ODataId>) -> Self
    where
        T: 'static,
    {
        Self::new(TypeId::of::<T>(), id)
    }
}

/// Broad direct relation kind between two Redfish resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RelationKind {
    /// The source resource is related to the target resource.
    RelatedTo,
    /// The source resource carries metrics for the target resource.
    MetricsFor,
}

/// Direct typed relation between two resources.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Relation {
    /// Source resource reference.
    pub from: ResourceRef,
    /// Target resource reference.
    pub to: ResourceRef,
    /// Relation kind.
    pub kind: RelationKind,
}

impl Relation {
    /// Creates a direct resource relation.
    #[must_use]
    pub const fn new(from: ResourceRef, to: ResourceRef, kind: RelationKind) -> Self {
        Self { from, to, kind }
    }
}
