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

use crate::edmx::ValidateError;
use crate::edmx::data_services::DataServices;
use crate::edmx::data_services::DeDataServices;
use crate::edmx::reference::Reference;
use serde::Deserialize;

/// 3.1 Element edmx:Edmx
#[derive(Debug, Deserialize)]
struct DeEdmx {
    /// 3.1.1 Attribute Version
    /// The edmx:Edmx element MUST provide the value 4.0 for the
    /// Version attribute.
    #[serde(rename = "@Version")]
    pub version: String,
    /// Child elements of Edmx.
    #[serde(rename = "$value", default)]
    pub items: Vec<DeEdmxItem>,
}

/// Child item of `edmx::Edmx`
#[derive(Debug, Deserialize)]
enum DeEdmxItem {
    /// edmx:Edmx element MUST contain a single direct child
    /// edmx:DataServices element.
    DataServices(DeDataServices),
    /// edmx:Edmx element contains zero or more edmx:Reference
    /// elements.
    Reference(Reference),
}

pub struct Edmx {
    pub data_services: DataServices,
    pub references: Vec<Reference>,
}

impl Edmx {
    /// # Errors
    /// Validation error or XML parsing error.
    pub fn parse(data: &str) -> Result<Self, ValidateError> {
        use quick_xml::de as quick_xml_de;
        quick_xml_de::from_str::<DeEdmx>(data)
            .map_err(ValidateError::XmlDeserialize)?
            .validate()
    }
}

impl DeEdmx {
    pub fn validate(self) -> Result<Edmx, ValidateError> {
        let (dss, refs) =
            self.items
                .into_iter()
                .fold((Vec::new(), Vec::new()), |(mut dss, mut refs), v| {
                    match v {
                        DeEdmxItem::DataServices(v) => dss.push(v),
                        DeEdmxItem::Reference(v) => refs.push(v),
                    }
                    (dss, refs)
                });

        // This element MUST contain a single direct child edmx:DataServices element.
        if dss.len() > 1 {
            return Err(ValidateError::WrongDataServicesNumber);
        }

        let ds = dss
            .into_iter()
            .next()
            .ok_or(ValidateError::WrongDataServicesNumber)?;

        Ok(Edmx {
            data_services: ds.validate()?,
            references: refs,
        })
    }
}
