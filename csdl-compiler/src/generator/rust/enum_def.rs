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

use crate::compiler::EnumType;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::generator::rust::Config;
use crate::generator::rust::TypeName;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use heck::AsUpperCamelCase;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::TokenStreamExt as _;
use quote::quote;

/// Type definition that maps to simple type.
#[derive(Debug)]
pub struct EnumDef<'a> {
    pub name: TypeName<'a>,
    pub compiled: EnumType<'a>,
}

impl EnumDef<'_> {
    /// Generate rust code for types derived from enums.
    pub fn generate(self, tokens: &mut TokenStream, _config: &Config) {
        let name = self.name;
        let mut members_content = TokenStream::new();
        for m in self.compiled.members {
            let rename = Literal::string(m.name.inner().inner());
            let member_name = EnumMemberName::new(m.name.inner());
            members_content.extend([
                doc_format_and_generate(m.name, &m.odata),
                quote! {
                    #[serde(rename=#rename)]
                    #member_name,
                },
            ]);
        }

        tokens.extend([
            doc_format_and_generate(self.name, &self.compiled.odata),
            quote! {
                #[derive(Serialize, Deserialize, Debug)]
                #[allow(clippy::enum_variant_names)]
                pub enum #name
            },
        ]);
        tokens.append(Group::new(Delimiter::Brace, members_content));
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
struct EnumMemberName<'a>(&'a SimpleIdentifier);

impl<'a> EnumMemberName<'a> {
    #[must_use]
    const fn new(v: &'a SimpleIdentifier) -> Self {
        Self(v)
    }
}

impl ToTokens for EnumMemberName<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new(
            &AsUpperCamelCase(self.0).to_string(),
            Span::call_site(),
        ));
    }
}
