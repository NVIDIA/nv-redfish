// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use nv_redfish_core::ODataId;

use crate::store::ResourceStore;
use crate::DiscoveryHint;
use crate::ResourceSnapshot;

/// Typed query predicate.
///
/// Discovery hints are optional optimizations. Snapshot matching is the
/// authoritative correctness check after candidates are fetched.
pub trait Predicate<T>: Send + Sync + 'static {
    /// Returns an optional discovery hint for this predicate.
    fn candidate_hint(&self) -> Option<DiscoveryHint>;

    /// Returns whether a candidate id can match this predicate.
    ///
    /// The default keeps candidates when a predicate has no candidate-stage
    /// filter.
    fn matches_candidate(&self, _id: &ODataId) -> bool {
        true
    }

    /// Returns whether a fetched snapshot matches this predicate.
    fn matches_snapshot(
        &self,
        snapshot: &ResourceSnapshot<T>,
        context: &PredicateContext<'_>,
    ) -> bool;
}

/// Store-backed context available to query predicates.
pub struct PredicateContext<'a> {
    store: &'a ResourceStore,
}

impl<'a> PredicateContext<'a> {
    pub(crate) const fn new(store: &'a ResourceStore) -> Self {
        Self { store }
    }

    /// Returns whether a source resource has any direct relation to target type
    /// `Target`.
    #[must_use]
    pub fn has_relation_to_type<Source, Target>(&self, source_id: &ODataId) -> bool
    where
        Source: 'static,
        Target: 'static,
    {
        self.store.has_relation_to_type::<Source, Target>(source_id)
    }
}

/// Resource-level predicates.
pub mod resource {
    use std::marker::PhantomData;

    use nv_redfish_core::ODataId;

    use crate::DiscoveryHint;
    use crate::Predicate;
    use crate::ResourceSnapshot;

    use super::PredicateContext;

    /// Starts a resource id predicate.
    #[must_use]
    pub const fn id() -> IdPredicateBuilder {
        IdPredicateBuilder
    }

    /// Matches resources directly related to resources of type `Target`.
    #[must_use]
    pub const fn related_to<Target>() -> RelatedToPredicate<Target>
    where
        Target: 'static,
    {
        RelatedToPredicate {
            target_type: PhantomData,
        }
    }

    /// Builder for resource id predicates.
    #[derive(Clone, Copy, Debug)]
    pub struct IdPredicateBuilder;

    impl IdPredicateBuilder {
        /// Matches resource ids containing `needle`.
        #[must_use]
        pub fn contains(self, needle: impl Into<String>) -> IdContainsPredicate {
            IdContainsPredicate {
                needle: needle.into(),
            }
        }
    }

    /// Predicate matching resource ids that contain a substring.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct IdContainsPredicate {
        needle: String,
    }

    impl IdContainsPredicate {
        fn matches_id(&self, id: &ODataId) -> bool {
            id.to_string().contains(&self.needle)
        }
    }

    impl<T> Predicate<T> for IdContainsPredicate
    where
        T: Send + Sync + 'static,
    {
        fn candidate_hint(&self) -> Option<DiscoveryHint> {
            Some(DiscoveryHint::id_contains(self.needle.clone()))
        }

        fn matches_candidate(&self, id: &ODataId) -> bool {
            self.matches_id(id)
        }

        fn matches_snapshot(
            &self,
            snapshot: &ResourceSnapshot<T>,
            _context: &PredicateContext<'_>,
        ) -> bool {
            self.matches_id(&snapshot.id)
        }
    }

    /// Predicate matching resources with a direct relation to target type `Target`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct RelatedToPredicate<Target> {
        target_type: PhantomData<fn() -> Target>,
    }

    impl<Source, Target> Predicate<Source> for RelatedToPredicate<Target>
    where
        Source: Send + Sync + 'static,
        Target: 'static,
    {
        fn candidate_hint(&self) -> Option<DiscoveryHint> {
            Some(DiscoveryHint::related_to::<Target>())
        }

        fn matches_snapshot(
            &self,
            snapshot: &ResourceSnapshot<Source>,
            context: &PredicateContext<'_>,
        ) -> bool {
            context.has_relation_to_type::<Source, Target>(&snapshot.id)
        }
    }
}

/// Sensor-specific predicates.
pub mod sensor {
    use nv_redfish::schema::physical_context::PhysicalContext;
    use nv_redfish::schema::sensor::ReadingType;
    use nv_redfish::schema::sensor::Sensor;
    use nv_redfish_core::ToSnakeCase as _;

    use crate::DiscoveryHint;
    use crate::Predicate;
    use crate::ResourceSnapshot;

    use super::PredicateContext;

    /// Starts a sensor reading type predicate.
    #[must_use]
    pub const fn reading_type() -> ReadingTypePredicateBuilder {
        ReadingTypePredicateBuilder
    }

    /// Starts a sensor physical context predicate.
    #[must_use]
    pub const fn physical_context() -> PhysicalContextPredicateBuilder {
        PhysicalContextPredicateBuilder
    }

    /// Starts a sensor name predicate.
    #[must_use]
    pub const fn name() -> NamePredicateBuilder {
        NamePredicateBuilder
    }

    /// Builder for sensor reading type predicates.
    #[derive(Clone, Copy, Debug)]
    pub struct ReadingTypePredicateBuilder;

    impl ReadingTypePredicateBuilder {
        /// Matches sensors with exactly this `ReadingType`.
        #[must_use]
        pub const fn equals(self, value: ReadingType) -> ReadingTypePredicate {
            ReadingTypePredicate { value }
        }
    }

    /// Predicate matching a sensor `ReadingType`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ReadingTypePredicate {
        value: ReadingType,
    }

    impl Predicate<Sensor> for ReadingTypePredicate {
        fn candidate_hint(&self) -> Option<DiscoveryHint> {
            Some(DiscoveryHint::semantic(format!(
                "sensor.reading_type={}",
                self.value.to_snake_case()
            )))
        }

        fn matches_snapshot(
            &self,
            snapshot: &ResourceSnapshot<Sensor>,
            _context: &PredicateContext<'_>,
        ) -> bool {
            snapshot
                .value
                .reading_type
                .as_ref()
                .and_then(Option::as_ref)
                .is_some_and(|value| value == &self.value)
        }
    }

    /// Builder for sensor physical context predicates.
    #[derive(Clone, Copy, Debug)]
    pub struct PhysicalContextPredicateBuilder;

    impl PhysicalContextPredicateBuilder {
        /// Matches sensors with exactly this `PhysicalContext`.
        #[must_use]
        pub const fn equals(self, value: PhysicalContext) -> PhysicalContextPredicate {
            PhysicalContextPredicate { value }
        }
    }

    /// Predicate matching a sensor `PhysicalContext`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PhysicalContextPredicate {
        value: PhysicalContext,
    }

    impl Predicate<Sensor> for PhysicalContextPredicate {
        fn candidate_hint(&self) -> Option<DiscoveryHint> {
            Some(DiscoveryHint::semantic(format!(
                "sensor.physical_context={}",
                self.value.to_snake_case()
            )))
        }

        fn matches_snapshot(
            &self,
            snapshot: &ResourceSnapshot<Sensor>,
            _context: &PredicateContext<'_>,
        ) -> bool {
            snapshot
                .value
                .physical_context
                .as_ref()
                .and_then(Option::as_ref)
                .is_some_and(|value| value == &self.value)
        }
    }

    /// Builder for sensor name predicates.
    #[derive(Clone, Copy, Debug)]
    pub struct NamePredicateBuilder;

    impl NamePredicateBuilder {
        /// Matches sensor names containing `needle`.
        #[must_use]
        pub fn contains(self, needle: impl Into<String>) -> NameContainsPredicate {
            NameContainsPredicate {
                needle: needle.into(),
            }
        }
    }

    /// Predicate matching a substring of `Sensor.Name`.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct NameContainsPredicate {
        needle: String,
    }

    impl Predicate<Sensor> for NameContainsPredicate {
        fn candidate_hint(&self) -> Option<DiscoveryHint> {
            Some(DiscoveryHint::semantic(format!(
                "sensor.name.contains={}",
                self.needle
            )))
        }

        fn matches_snapshot(
            &self,
            snapshot: &ResourceSnapshot<Sensor>,
            _context: &PredicateContext<'_>,
        ) -> bool {
            snapshot.value.base.name.contains(&self.needle)
        }
    }
}
