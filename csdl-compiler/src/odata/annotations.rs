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

//! Helper of handling annotations in edmx types.

use crate::edmx::Parameter;
use crate::edmx::action::Action;
use crate::edmx::annotation::Annotation;
use crate::edmx::annotation::AnnotationRecord;
use crate::edmx::attribute_values::Namespace;
use crate::edmx::complex_type::ComplexType;
use crate::edmx::entity_type::EntityType;
use crate::edmx::enum_type::EnumMember;
use crate::edmx::enum_type::EnumType;
use crate::edmx::property::DeStructuralProperty;
use crate::edmx::property::NavigationProperty;
use tagged_types::TaggedType;

pub type Description = TaggedType<String, DescriptionTag>;
pub type DescriptionRef<'a> = TaggedType<&'a String, DescriptionTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy)]
#[transparent(Display, Debug)]
#[capability(inner_access, cloned)]
pub enum DescriptionTag {}

pub type LongDescription = TaggedType<String, LongDescriptionTag>;
pub type LongDescriptionRef<'a> = TaggedType<&'a String, LongDescriptionTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone, Copy)]
#[transparent(Display, Debug)]
#[capability(inner_access, cloned)]
pub enum LongDescriptionTag {}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissions {
    #[default]
    Read,
    Write,
    ReadWrite,
}

trait IsODataNamespace {
    fn is_odata_namespace(&self) -> bool;
}

impl IsODataNamespace for Namespace {
    fn is_odata_namespace(&self) -> bool {
        self.ids.len() == 1 && self.ids[0].inner() == "OData"
    }
}

pub trait ODataAnnotation {
    fn is_odata_annotation(&self, name: &str) -> bool;
}

impl ODataAnnotation for Annotation {
    fn is_odata_annotation(&self, name: &str) -> bool {
        self.term.inner().namespace.is_odata_namespace() && self.term.inner().name.inner() == name
    }
}

pub trait ODataAnnotations {
    fn annotations(&self) -> &Vec<Annotation>;

    fn odata_description(&self) -> Option<DescriptionRef<'_>> {
        self.annotations()
            .iter()
            .find(|a| a.is_odata_annotation("Description"))
            .and_then(|a| a.string.as_ref())
            .map(DescriptionRef::new)
    }

    fn odata_long_description(&self) -> Option<LongDescriptionRef<'_>> {
        self.annotations()
            .iter()
            .find(|a| a.is_odata_annotation("LongDescription"))
            .and_then(|a| a.string.as_ref())
            .map(LongDescriptionRef::new)
    }

    fn odata_permissions(&self) -> Option<Permissions> {
        self.annotations()
            .iter()
            .find(|a| a.is_odata_annotation("Permissions"))
            .and_then(|a| a.enum_member.as_ref())
            .and_then(|v| match v.mname.inner().inner().as_str() {
                "ReadWrite" => Some(Permissions::ReadWrite),
                "Read" => Some(Permissions::Read),
                "Write" => Some(Permissions::Write),
                _ => None,
            })
    }
}

impl ODataAnnotations for EnumType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for EnumMember {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for EntityType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for ComplexType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for DeStructuralProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for NavigationProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for AnnotationRecord {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for Action {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl ODataAnnotations for Parameter {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}
