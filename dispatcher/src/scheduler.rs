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

//! The unified [`Scheduler`] trait — every node in the scheduling tree
//! implements it.
//!
//! Leaves produce work directly; branches compose children with a policy.
//! The runtime treats both alike: query readiness, pull one item, forward
//! the completion. `Scheduler<T>` is generic over the payload `T` and
//! never inspects it; the runtime decides what shape `T` takes (the
//! bundled [`crate::Runtime`] uses [`crate::FutureWork<Ev, Err>`]).

use std::time::Instant;

use crate::work::Completion;
use crate::work::Readiness;
use crate::work::RoutingPath;
use crate::work::WorkMeta;

/// Unit of work returned by [`Scheduler::take_next`].
pub struct ScheduledWork<T, M: WorkMeta> {
    /// Meta as observed at the producing node.
    pub meta: M,
    /// Breadcrumb stack stamped by branches on the way up. See
    /// [`RoutingPath`].
    pub routing: RoutingPath,
    /// Opaque payload; only the runtime executes it.
    pub payload: T,
}

impl<T, M: WorkMeta> ScheduledWork<T, M> {
    /// Build a [`ScheduledWork`] with an empty routing path. Typical for
    /// leaves; branches re-use the child's path and stamp their own tag.
    #[must_use]
    pub const fn new(meta: M, payload: T) -> Self {
        Self {
            meta,
            routing: RoutingPath::empty(),
            payload,
        }
    }
}

/// Composable scheduler node, parameterized by the opaque payload `T`.
///
/// The runtime drives the root: refresh readiness, pull a payload when
/// admission permits, report completion exactly once. Branches recurse
/// into their selected child and may stamp the routing path and/or wrap
/// the child's meta on the way up; on completion they pop their tag,
/// unwrap their layer, and forward.
///
/// `Send + 'static` is required so the runtime can store a node behind a
/// trait object and downcast it via [`core::any::Any`].
pub trait Scheduler<T>: Send + 'static {
    /// Meta produced at `take_next` and consumed at `on_complete`. `()`
    /// for meta-naive leaves; branches wrap the child meta with a layer
    /// like [`crate::WithCost`].
    type Meta: WorkMeta;

    /// Refresh readiness against `now`. Branches aggregate across
    /// children (ready iff any child is ready; `next_update_at` is the
    /// min; `next_cost` follows the branch policy).
    fn update_ready(&mut self, now: Instant) -> Readiness;

    /// Pull the next item, or `None` if nothing is currently available.
    ///
    /// Branches must push the selected child index onto `work.routing`
    /// before returning, and replace `work.meta` if they add a layer.
    fn take_next(&mut self) -> Option<ScheduledWork<T, Self::Meta>>;

    /// Report the completion of a dispatched item, exactly once.
    ///
    /// Branches pop their routing tag from `completion.routing`, read
    /// their layer's annotations off `completion.meta`, and forward a
    /// `&mut Completion<C::Meta>` (with the unwrapped meta) to the chosen
    /// child.
    fn on_complete(&mut self, completion: &mut Completion<Self::Meta>);
}

impl<T, S> Scheduler<T> for Box<S>
where
    T: 'static,
    S: Scheduler<T> + ?Sized,
{
    type Meta = S::Meta;

    fn update_ready(&mut self, now: Instant) -> Readiness {
        (**self).update_ready(now)
    }

    fn take_next(&mut self) -> Option<ScheduledWork<T, S::Meta>> {
        (**self).take_next()
    }

    fn on_complete(&mut self, completion: &mut Completion<S::Meta>) {
        (**self).on_complete(completion);
    }
}

pub(crate) mod private {
    //! Sealed object-safe extension of [`super::Scheduler`] used as the
    //! runtime's internal root type. Adopts every [`super::Scheduler`] via
    //! a blanket impl; users never implement it.

    use core::any::Any;
    use std::time::Instant;

    use super::ScheduledWork;
    use crate::work::Completion;
    use crate::work::Readiness;
    use crate::work::WorkMeta;

    /// Object-safe scheduler view used by the runtime's internal storage.
    pub trait SchedulerObj<T, M: WorkMeta>: Send + 'static {
        fn update_ready(&mut self, now: Instant) -> Readiness;
        fn take_next(&mut self) -> Option<ScheduledWork<T, M>>;
        fn on_complete(&mut self, completion: &mut Completion<M>);
        fn as_any(&self) -> &dyn Any;
        fn as_any_mut(&mut self) -> &mut dyn Any;
    }

    impl<T, M, S> SchedulerObj<T, M> for S
    where
        T: 'static,
        M: WorkMeta,
        S: super::Scheduler<T, Meta = M>,
    {
        fn update_ready(&mut self, now: Instant) -> Readiness {
            <Self as super::Scheduler<T>>::update_ready(self, now)
        }
        fn take_next(&mut self) -> Option<ScheduledWork<T, M>> {
            <Self as super::Scheduler<T>>::take_next(self)
        }
        fn on_complete(&mut self, completion: &mut Completion<M>) {
            <Self as super::Scheduler<T>>::on_complete(self, completion);
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }
}
