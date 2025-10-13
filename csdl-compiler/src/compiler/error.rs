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

use crate::compiler::QualifiedName;
use crate::edmx::ActionName;
use crate::edmx::Namespace;
use crate::edmx::ParameterName;
use crate::edmx::PropertyName;
use crate::edmx::SimpleIdentifier;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Compilation error.
#[derive(Debug)]
pub enum Error<'a> {
    Unimplemented,
    NotBoundAction,
    NoBindingParameterForAction,
    EntityTypeNotFound(QualifiedName<'a>),
    ComplexTypeNotFound(QualifiedName<'a>),
    SettingsTypeNotFound,
    SettingsPreferredApplyTimeTypeNotFound,
    EntityType(QualifiedName<'a>, Box<Error<'a>>),
    TypeNotFound(QualifiedName<'a>),
    TypeDefinitionOfNotPrimitiveType(QualifiedName<'a>),
    TypeDefinition(QualifiedName<'a>, Box<Error<'a>>),
    Type(QualifiedName<'a>, Box<Error<'a>>),
    Property(&'a PropertyName, Box<Error<'a>>),
    Action(&'a ActionName, Box<Error<'a>>),
    ActionReturnType(Box<Error<'a>>),
    ActionParameter(&'a ParameterName, Box<Error<'a>>),
    Singleton(&'a SimpleIdentifier, Box<Error<'a>>),
    Schema(&'a Namespace, Box<Error<'a>>),
}

impl Display for Error<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Unimplemented => writeln!(f, "unimplemented"),
            Self::EntityTypeNotFound(v) => writeln!(f, "entity type not found: {v}"),
            Self::ComplexTypeNotFound(v) => writeln!(f, "complex type not found: {v}"),
            Self::SettingsTypeNotFound => writeln!(
                f,
                "cannot find type for redfish settings (Settings.Settings)"
            ),
            Self::SettingsPreferredApplyTimeTypeNotFound => writeln!(
                f,
                "cannot find type for redfish settings preferred apply time (Settings.PreferredApplyTime)"
            ),
            Self::NotBoundAction => {
                write!(f, "unbound action is not supported")
            }
            Self::NoBindingParameterForAction => {
                write!(f, "no required binding parameter")
            }
            Self::EntityType(name, err) => {
                write!(f, "while compiling entity type: {name}\n{err}")
            }
            Self::TypeNotFound(v) => writeln!(f, "type not found: {v}"),
            Self::TypeDefinitionOfNotPrimitiveType(v) => {
                write!(f, "type definition is not primitive type: {v}")
            }
            Self::TypeDefinition(name, err) => {
                write!(f, "while compiling type definition: {name}\n{err}")
            }
            Self::Type(name, err) => {
                write!(f, "while compiling type: {name}\n{err}")
            }
            Self::Property(name, err) => {
                write!(f, "while compiling property: {name}\n{err}")
            }
            Self::Action(name, err) => {
                write!(f, "while compiling action: {name}\n{err}")
            }
            Self::ActionReturnType(err) => {
                write!(f, "while compiling return type\n{err}")
            }
            Self::ActionParameter(name, err) => {
                write!(f, "while compiling parameter: {name}\n{err}")
            }
            Self::Singleton(name, err) => write!(f, "while compiling singleton: {name}\n{err}"),
            Self::Schema(name, err) => write!(f, "while compiling schema: {name}\n{err}"),
        }
    }
}
