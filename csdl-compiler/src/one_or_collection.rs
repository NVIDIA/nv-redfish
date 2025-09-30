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

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// One item or collection of items for types.
///
/// This is common construction in compiler when we need to describe
/// singleton or collection of items of specific type.
pub enum OneOrCollection<T> {
    One(T),
    Collection(T),
}

impl<T> OneOrCollection<T> {
    /// Inner type.
    #[must_use]
    pub const fn inner(&self) -> &T {
        match self {
            Self::One(v) | Self::Collection(v) => v,
        }
    }
}

impl<T> OneOrCollection<T> {
    /// Maps inner value with funciton `f`.
    pub fn map<F, R>(self, f: F) -> OneOrCollection<R>
    where
        F: FnOnce(T) -> R,
    {
        match self {
            Self::One(v) => OneOrCollection::<R>::One(f(v)),
            Self::Collection(v) => OneOrCollection::<R>::Collection(f(v)),
        }
    }

    /// Convert from `OneOrCollection<T>` to `OneOrCollection<&T>`.
    #[inline]
    pub const fn as_ref(&self) -> OneOrCollection<&T> {
        match self {
            Self::One(v) => OneOrCollection::<&T>::One(v),
            Self::Collection(v) => OneOrCollection::<&T>::Collection(v),
        }
    }
}

impl<T: Debug> Debug for OneOrCollection<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::One(v) => write!(f, "One({v:?})"),
            Self::Collection(v) => write!(f, "Collection({v:?})"),
        }
    }
}

// This is generic implementation based on what T is implementing.  We
// are fine with exact copy on clone but if T implements Clone without
// Copy we still want to have clone.
#[allow(clippy::expl_impl_clone_on_copy)]
impl<T: Clone> Clone for OneOrCollection<T> {
    fn clone(&self) -> Self {
        match self {
            Self::One(v) => Self::One(v.clone()),
            Self::Collection(v) => Self::Collection(v.clone()),
        }
    }
}

impl<T: Copy> Copy for OneOrCollection<T> {}

impl<T: PartialEq> PartialEq for OneOrCollection<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::One(v1), Self::One(v2)) | (Self::Collection(v1), Self::Collection(v2)) => {
                v1.eq(v2)
            }
            _ => false,
        }
    }
}

impl<T: Eq> Eq for OneOrCollection<T> {}
