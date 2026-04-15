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

use crate::edmx::ActionName;
use crate::edmx::LocalTypeName;
use crate::edmx::Namespace;
use crate::edmx::PropertyName;
use quick_xml::DeError;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// EDMX compilation errors.
#[derive(Debug)]
pub enum ValidateError {
    /// XML deserialization error.
    XmlDeserialize(DeError),
    /// Invalid number of `DataServices`.
    WrongDataServicesNumber,
    /// In the `EntityType` too many keys.
    TooManyKeys,
    /// In the `NavigationProperty` too many `OnDelete` items.
    TooManyOnDelete,
    /// In the `Action` too many `ReturnType` items.
    TooManyReturnTypes,
    /// Not supported more than one entity container in Schema.
    /// This is the case for Redfish. Keep it this way for parser.
    ManyContainersNotSupported,
    /// Schema validation error.
    Schema(Namespace, Box<Self>),
    /// `ComplexType` validation error.
    ComplexType(LocalTypeName, Box<Self>),
    /// `EntityType` validation error.
    EntityType(LocalTypeName, Box<Self>),
    /// `NavigationProperty` validation error.
    NavigationProperty(PropertyName, Box<Self>),
    /// `Action` validation error.
    Action(ActionName, Box<Self>),
}

impl Display for ValidateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::XmlDeserialize(error) => write!(f, "xml deserialization error: {error}"),
            Self::WrongDataServicesNumber => write!(
                f,
                "wrong number of data services in xml (only one must be specified)"
            ),
            Self::TooManyKeys => write!(f, "too many Key elements in EntityType"),
            Self::TooManyOnDelete => write!(f, "too many OnDelete elements"),
            Self::TooManyReturnTypes => write!(f, "too many ReturnType elements in Action"),
            Self::ManyContainersNotSupported => {
                write!(f, "more than one entity container per schema not supported")
            }
            Self::Schema(ns, err) => write!(f, "schema {ns} validation error: {err}"),
            Self::ComplexType(ct, err) => write!(f, "complex type {ct} validation error: {err}"),
            Self::EntityType(et, err) => write!(f, "entity type {et} validation error: {err}"),
            Self::NavigationProperty(pn, err) => {
                write!(f, "navigation property {pn} validation error: {err}")
            }
            Self::Action(an, err) => write!(f, "action {an} validation error: {err}"),
        }
    }
}

impl StdError for ValidateError {}
