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

use crate::edmx::Annotation;
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
#[implement(Clone)]
#[transparent(Display, Debug)]
#[capability(inner_access, cloned)]
pub enum DescriptionTag {}

pub type LongDescription = TaggedType<String, LongDescriptionTag>;
pub type LongDescriptionRef<'a> = TaggedType<&'a String, LongDescriptionTag>;
#[derive(tagged_types::Tag)]
#[implement(Clone)]
#[transparent(Display, Debug)]
#[capability(inner_access, cloned)]
pub enum LongDescriptionTag {}

pub trait ODataAnnotations {
    fn annotations(&self) -> &Vec<Annotation>;

    fn default_description(&self) -> Description;

    fn odata_description(&self) -> Option<DescriptionRef<'_>> {
        self.annotations()
            .iter()
            .find(|a| a.term == "OData.Description")
            .and_then(|a| a.string.as_ref())
            .map(DescriptionRef::new)
    }

    fn odata_description_or_default(&self) -> Description {
        self.odata_description()
            .map_or_else(|| self.default_description(), TaggedType::cloned)
    }

    fn odata_long_description(&self) -> Option<LongDescriptionRef<'_>> {
        self.annotations()
            .iter()
            .find(|a| a.term == "OData.LongDescription")
            .and_then(|a| a.string.as_ref())
            .map(LongDescriptionRef::new)
    }
}

impl ODataAnnotations for EnumType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("Enum {}", self.name))
    }
}

impl ODataAnnotations for EnumMember {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("EnumMember {}", self.name))
    }
}

impl ODataAnnotations for EntityType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("Resource {}", self.name))
    }
}

impl ODataAnnotations for ComplexType {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("Complex type {}", self.name))
    }
}

impl ODataAnnotations for DeStructuralProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("Property {}", self.name))
    }
}

impl ODataAnnotations for NavigationProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }

    fn default_description(&self) -> Description {
        Description::new(format!("Navigation property {}", self.name))
    }
}
