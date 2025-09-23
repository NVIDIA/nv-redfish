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

use crate::compiler::Action;
use crate::compiler::ActionsMap;
use crate::compiler::ComplexType;
use crate::compiler::EntityType;
use crate::compiler::EnumType;
use crate::compiler::OData;
use crate::compiler::Parameter;
use crate::compiler::Properties;
use crate::compiler::QualifiedName;
use crate::compiler::TypeDefinition;
use crate::generator::rust::Config;
use crate::generator::rust::EnumDef;
use crate::generator::rust::Error;
use crate::generator::rust::ModName;
use crate::generator::rust::StructDef;
use crate::generator::rust::TypeDef;
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
    typedefs: HashMap<TypeName<'a>, TypeDef<'a>>,
    enums: HashMap<TypeName<'a>, EnumDef<'a>>,
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
            enums: HashMap::new(),
            typedefs: HashMap::new(),
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
        ct: ComplexType<'a>,
        actions: ActionsMap<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        self.inner_add_complex_type(ct, 0, actions, config)
    }

    fn inner_add_complex_type(
        mut self,
        ct: ComplexType<'a>,
        depth: usize,
        actions: ActionsMap<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = ct.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_complex_type(ct, depth + 1, actions, config)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let struct_name = TypeName::new_qualified(ct.name.name);
            self.add_struct_def(
                struct_name,
                ct.base,
                ct.properties,
                Vec::new(),
                ct.odata,
                actions,
                config,
            )
            .map_err(Box::new)
            .map_err(|e| Error::CreateStruct(struct_name, e))
        }
    }

    /// Add enum type to the module.
    ///
    /// # Errors
    ///
    /// TODO
    pub fn add_enum_type(self, t: EnumType<'a>) -> Result<Self, Error<'a>> {
        self.inner_add_enum_type(t, 0)
    }

    fn inner_add_enum_type(mut self, t: EnumType<'a>, depth: usize) -> Result<Self, Error<'a>> {
        if let Some(id) = t.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_enum_type(t, depth + 1)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let name = t.name;
            let type_name = TypeName::new_qualified(t.name.name);
            match self.enums.entry(type_name) {
                Entry::Occupied(_) => Err(Error::NameConflict),
                Entry::Vacant(v) => {
                    v.insert(EnumDef {
                        name: type_name,
                        compiled: t,
                    });
                    Ok(self)
                }
            }
            .map_err(Box::new)
            .map_err(|e| Error::CreateSimplType(name, e))
        }
    }

    /// Add type definition to the module.
    ///
    /// # Errors
    ///
    ///
    pub fn add_type_definition(self, t: TypeDefinition<'a>) -> Result<Self, Error<'a>> {
        self.inner_add_type_definition(t, 0)
    }

    fn inner_add_type_definition(
        mut self,
        t: TypeDefinition<'a>,
        depth: usize,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = t.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_type_definition(t, depth + 1)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let name = t.name;
            let type_name = TypeName::new_qualified(t.name.name);
            match self.typedefs.entry(type_name) {
                Entry::Occupied(_) => Err(Error::NameConflict),
                Entry::Vacant(v) => {
                    v.insert(TypeDef {
                        name: type_name,
                        compiled: t,
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
    pub fn add_entity_type(self, t: EntityType<'a>, config: &Config) -> Result<Self, Error<'a>> {
        self.inner_add_entity_type(t, 0, config)
    }

    fn inner_add_entity_type(
        mut self,
        t: EntityType<'a>,
        depth: usize,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = t.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_entity_type(t, depth + 1, config)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let struct_name = TypeName::new_qualified(t.name.name);
            self.add_struct_def(
                struct_name,
                t.base,
                t.properties,
                Vec::new(),
                t.odata,
                ActionsMap::default(),
                config,
            )
            .map_err(Box::new)
            .map_err(|e| Error::CreateStruct(struct_name, e))
        }
    }

    /// Add complex type to the module.
    ///
    /// # Errors
    ///
    /// Returns `CreateStruct` error if failed to add new struct to the
    /// module.  it may only happen in case of name conflicts because
    /// of case conversion.
    pub fn add_action_type(self, t: &Action<'a>, config: &Config) -> Result<Self, Error<'a>> {
        self.inner_add_action_type(t, 0, config)
    }

    fn inner_add_action_type(
        mut self,
        t: &Action<'a>,
        depth: usize,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        if let Some(id) = t.binding.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name, depth))
                .inner_add_action_type(t, depth + 1, config)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let struct_name = TypeName::new_action(t.binding_name, t.name);
            self.add_struct_def(
                struct_name,
                None,
                Properties::default(),
                t.parameters.clone(),
                t.odata,
                HashMap::new(),
                config,
            )
            .map_err(Box::new)
            .map_err(|e| Error::CreateStruct(struct_name, e))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_struct_def(
        mut self,
        struct_name: TypeName<'a>,
        base: Option<QualifiedName<'a>>,
        properties: Properties<'a>,
        parameters: Vec<Parameter<'a>>,
        odata: OData<'a>,
        actions: ActionsMap<'a>,
        config: &Config,
    ) -> Result<Self, Error<'a>> {
        match self.structs.entry(struct_name) {
            Entry::Occupied(_) => Err(Error::NameConflict),
            Entry::Vacant(v) => {
                StructDef::new(
                    struct_name,
                    base,
                    properties,
                    parameters,
                    odata,
                    actions,
                    config,
                )
                .map(|st| v.insert(st))?;
                Ok(self)
            }
        }
    }

    /// Generate Rust code.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        let mut typedefs = self.typedefs.into_values().collect::<Vec<_>>();
        typedefs.sort_by_key(|v| v.name);

        let mut enums = self.enums.into_values().collect::<Vec<_>>();
        enums.sort_by_key(|v| v.name);

        let mut sub_mods = self.sub_mods.into_values().collect::<Vec<_>>();
        sub_mods.sort_by_key(|v| v.name);

        let mut structs = self.structs.into_values().collect::<Vec<_>>();
        structs.sort_by_key(|v| v.name);

        let generate = |ts: &mut TokenStream| {
            for t in typedefs {
                t.generate(ts, config);
            }

            for t in enums {
                t.generate(ts, config);
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
                    use serde::{Serialize, Deserialize};
                    #[allow(unused_imports)]
                    use #top::{NavProperty, ODataId};
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
