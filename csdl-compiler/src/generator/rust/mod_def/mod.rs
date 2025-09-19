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

pub mod name;

use crate::compiler::CompiledComplexType;
use crate::compiler::CompiledEntityType;
use crate::compiler::CompiledOData;
use crate::compiler::CompiledProperties;
use crate::compiler::QualifiedName;
use crate::compiler::SimpleType;
use crate::generator::rust::Config;
use crate::generator::rust::Error;
use crate::generator::rust::ModName;
use crate::generator::rust::SimpleDef;
use crate::generator::rust::StructDef;
use crate::generator::rust::TypeName;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Ident;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::repeat_n;

#[derive(Default, Debug)]
pub struct ModDef<'a> {
    name: Option<ModName<'a>>,
    simples: HashMap<TypeName<'a>, SimpleDef<'a>>,
    structs: HashMap<TypeName<'a>, StructDef<'a>>,
    sub_mods: HashMap<ModName<'a>, ModDef<'a>>,
    depth: usize,
}

impl<'a> ModDef<'a> {
    #[must_use]
    pub fn new(name: ModName<'a>, depth: usize) -> Self {
        Self {
            name: Some(name),
            structs: HashMap::new(),
            sub_mods: HashMap::new(),
            simples: HashMap::new(),
            depth,
        }
    }

    /// Add complex type to the module.
    ///
    /// # Errors
    ///
    /// Returns `CreateStruct` error if failed to add new struct to the
    /// module.  it may only happen in case of name conflicts because
    /// of case conversion.
    pub fn add_complex_type(
        self,
        ct: CompiledComplexType<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        self.inner_add_complex_type(ct, 0, config)
    }

    fn inner_add_complex_type(
        mut self,
        ct: CompiledComplexType<'a>,
        depth: usize,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = ct.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_complex_type(ct, depth + 1, config)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let name = ct.name;
            self.add_struct_def(ct.name, ct.base, ct.properties, ct.odata, config)
                .map_err(Box::new)
                .map_err(|e| Error::CreateStruct(name, e))
        }
    }

    /// Add simple type to the module.
    ///
    /// # Errors
    ///
    /// TODO
    pub fn add_simple_type(self, ct: SimpleType<'a>) -> Result<Self, Error<'a>> {
        self.inner_add_simple_type(ct, 0)
    }

    fn inner_add_simple_type(
        mut self,
        st: SimpleType<'a>,
        depth: usize,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = st.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_simple_type(st, depth + 1)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let name = st.name;
            let type_name = TypeName::new(st.name.name);
            let attrs = st.attrs;
            match self.simples.entry(type_name) {
                Entry::Occupied(_) => Err(Error::NameConflict),
                Entry::Vacant(v) => {
                    v.insert(SimpleDef {
                        name: type_name,
                        attrs,
                    });
                    Ok(self)
                }
            }
            .map_err(Box::new)
            .map_err(|e| Error::CreateSimplType(name, e))
        }
    }

    /// Add entity type to the module.
    ///
    /// # Errors
    ///
    /// Returns `CreateStruct` error if failed to add new struct to the
    /// module.  it may only happen in case of name conflicts because
    /// of case conversion.
    pub fn add_entity_type(
        self,
        ct: CompiledEntityType<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        self.inner_add_entity_type(ct, 0, config)
    }

    fn inner_add_entity_type(
        mut self,
        et: CompiledEntityType<'a>,
        depth: usize,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = et.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_entity_type(et, depth + 1, config)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let name = et.name;
            self.add_struct_def(et.name, et.base, et.properties, et.odata, config)
                .map_err(Box::new)
                .map_err(|e| Error::CreateStruct(name, e))
        }
    }

    fn add_struct_def(
        mut self,
        qname: QualifiedName<'a>,
        base: Option<QualifiedName<'a>>,
        properties: CompiledProperties<'a>,
        odata: CompiledOData<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        let struct_name = TypeName::new(qname.name);
        match self.structs.entry(struct_name) {
            Entry::Occupied(_) => Err(Error::NameConflict),
            Entry::Vacant(v) => {
                StructDef::new(struct_name, base, properties, odata, config)
                    .map(|st| v.insert(st))?;
                Ok(self)
            }
        }
    }

    /// Generate Rust code.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        let mut simples = self.simples.into_values().collect::<Vec<_>>();
        simples.sort_by_key(|v| v.name);

        let mut sub_mods = self.sub_mods.into_values().collect::<Vec<_>>();
        sub_mods.sort_by_key(|v| v.name);

        let mut structs = self.structs.into_values().collect::<Vec<_>>();
        structs.sort_by_key(|v| v.name);

        let generate = |ts: &mut TokenStream| {
            for s in simples {
                s.generate(ts, config);
            }

            for s in structs {
                s.generate(ts, config);
            }

            for m in sub_mods {
                m.generate(ts, config);
            }
        };

        if let Some(name) = self.name {
            let top = &config.top_module_alias;
            let mut content = TokenStream::new();
            content.extend([
                Self::generate_ref_to_top_module(self.depth, config),
                quote! {
                    #[allow(unused_imports)]
                    use serde::Deserialize;
                    #[allow(unused_imports)]
                    use #top::NavProperty;
                },
            ]);
            generate(&mut content);
            tokens.extend([
                quote! {
                    pub mod #name
                },
                TokenTree::Group(Group::new(Delimiter::Brace, content)).into(),
            ]);
        } else {
            generate(tokens);
        }
    }

    fn generate_ref_to_top_module(depth: usize, config: &Config) -> TokenStream {
        let top = &config.top_module_alias;
        let supers = repeat_n(
            [
                TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                TokenTree::Ident(Ident::new("super", Span::call_site())),
            ],
            depth + 1,
        )
        .flatten()
        .collect::<Vec<_>>();
        let mut ts = TokenStream::new();
        ts.extend(quote! {
            #[allow(unused_imports)]
            use super
        });
        ts.extend(supers);
        ts.extend(quote! { ::#top  as #top ; });
        ts
    }
}
