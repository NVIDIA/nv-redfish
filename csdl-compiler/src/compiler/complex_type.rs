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

use crate::compiler::MapBase;
use crate::compiler::NavProperty;
use crate::compiler::OData;
use crate::compiler::Properties;
use crate::compiler::PropertiesManipulation;
use crate::compiler::Property;
use crate::compiler::QualifiedName;

#[derive(Debug)]
pub struct ComplexType<'a> {
    pub name: QualifiedName<'a>,
    pub base: Option<QualifiedName<'a>>,
    pub properties: Properties<'a>,
    pub odata: OData<'a>,
}

impl<'a> PropertiesManipulation<'a> for ComplexType<'a> {
    fn map_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(Property<'a>) -> Property<'a>,
    {
        self.properties.properties = self.properties.properties.into_iter().map(f).collect();
        self
    }

    fn map_nav_properties<F>(mut self, f: F) -> Self
    where
        F: Fn(NavProperty<'a>) -> NavProperty<'a>,
    {
        self.properties.nav_properties =
            self.properties.nav_properties.into_iter().map(f).collect();
        self
    }
}

impl<'a> MapBase<'a> for ComplexType<'a> {
    fn map_base<F>(mut self, f: F) -> Self
    where
        F: FnOnce(QualifiedName<'a>) -> QualifiedName<'a>,
    {
        self.base = self.base.map(f);
        self
    }
}
