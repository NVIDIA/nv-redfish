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

use crate::compiler::NavProperty;
use crate::compiler::Property;
use crate::compiler::QualifiedName;

/// Update properties and navigation properties with specified
/// function.
pub trait PropertiesManipulation<'a> {
    #[must_use]
    fn map_properties<F>(self, f: F) -> Self
    where
        F: Fn(Property<'a>) -> Property<'a>;

    #[must_use]
    fn map_nav_properties<F>(self, f: F) -> Self
    where
        F: Fn(NavProperty<'a>) -> NavProperty<'a>;
}

/// Update type with specified function.
pub trait MapType<'a> {
    #[must_use]
    fn map_type<F>(self, f: F) -> Self
    where
        F: Fn(QualifiedName<'a>) -> QualifiedName<'a>;
}

/// Update base type with specified function.
pub trait MapBase<'a> {
    #[must_use]
    fn map_base<F>(self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>;
}
