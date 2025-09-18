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

use crate::compiler::CompiledOData;
use crate::compiler::CompiledProperties;
use crate::compiler::CompiledPropertyType;
use crate::generator::rust::Config;
use crate::generator::rust::FullTypeName;
use crate::generator::rust::PropertyName;
use crate::generator::rust::TypeName;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Literal;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;

/// Definition of Rust struct.
#[derive(Debug)]
pub struct StructDef<'a> {
    pub name: TypeName<'a>,
    pub properties: CompiledProperties<'a>,
    pub odata: CompiledOData<'a>,
}

impl StructDef<'_> {
    /// Generate rust code for the structure.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        let mut content = TokenStream::new();
        for p in self.properties.properties {
            match p.ptype {
                CompiledPropertyType::One(v) => {
                    let rename = Literal::string(p.name.inner().inner());
                    let name = PropertyName::new(p.name);
                    let ptype = FullTypeName::new(v, config);
                    content.extend([
                        doc_format_and_generate(p.name, &p.odata),
                        quote! {
                            #[serde(rename=#rename)]
                            pub #name: Option<#ptype>,
                        },
                    ]);
                }
                CompiledPropertyType::CollectionOf(v) => {
                    let rename = Literal::string(p.name.inner().inner());
                    let name = PropertyName::new(p.name);
                    let ptype = FullTypeName::new(v, config);
                    content.extend([
                        doc_format_and_generate(p.name, &p.odata),
                        quote! {
                            #[serde(rename=#rename, default)]
                            pub #name: Vec<#ptype>,
                        },
                    ]);
                }
            }
        }
        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Deserialize)]
                pub struct #name
            },
            TokenTree::Group(Group::new(Delimiter::Brace, content)).into(),
        ]);
    }
}
