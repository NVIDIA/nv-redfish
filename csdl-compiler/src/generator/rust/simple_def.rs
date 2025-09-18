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

use crate::compiler::SimpleTypeAttrs;
use crate::generator::rust::Config;
use crate::generator::rust::FullTypeName;
use crate::generator::rust::TypeName;
use proc_macro2::TokenStream;
use quote::quote;

/// Type definition that maps to simple type.
#[derive(Debug)]
pub struct SimpleDef<'a> {
    pub name: TypeName<'a>,
    pub attrs: SimpleTypeAttrs<'a>,
}

impl SimpleDef<'_> {
    /// Generate rust code for the structure.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        let name = self.name;
        match self.attrs {
            SimpleTypeAttrs::TypeDefinition(td) => {
                let underlying_type = FullTypeName::new(td.underlying_type, config);
                tokens.extend(quote! {
                    pub type #name = #underlying_type;
                });
            }
            SimpleTypeAttrs::EnumType(_) => {
                // TODO: members
                tokens.extend(quote! {
                    pub type #name = i32;
                });
            }
        }
    }
}
