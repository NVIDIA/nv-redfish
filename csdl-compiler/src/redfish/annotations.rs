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

use crate::IsRequired;
use crate::IsRequiredOnCreate;
use crate::edmx::Annotation;
use crate::edmx::NavigationProperty;
use crate::edmx::Parameter;
use crate::edmx::StructuralProperty;

pub trait RedfishAnnotation {
    fn is_redfish_annotation(&self, name: &str) -> bool;
}

impl RedfishAnnotation for Annotation {
    fn is_redfish_annotation(&self, name: &str) -> bool {
        self.term.inner().namespace.ids.len() == 1
            && self.term.inner().namespace.ids[0].inner() == "Redfish"
            && self.term.inner().name.inner() == name
    }
}

pub trait RedfishPropertyAnnotations {
    fn annotations(&self) -> &Vec<Annotation>;

    fn is_required(&self) -> IsRequired {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("Required"))
            .map_or_else(|| IsRequired::new(false), |_| IsRequired::new(true))
    }

    fn is_required_on_create(&self) -> IsRequiredOnCreate {
        self.annotations()
            .iter()
            .find(|a| a.is_redfish_annotation("RequiredOnCreate"))
            .map_or_else(
                || IsRequiredOnCreate::new(false),
                |_| IsRequiredOnCreate::new(true),
            )
    }
}

impl RedfishPropertyAnnotations for StructuralProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishPropertyAnnotations for NavigationProperty {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}

impl RedfishPropertyAnnotations for Parameter {
    fn annotations(&self) -> &Vec<Annotation> {
        &self.annotations
    }
}
