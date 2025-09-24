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
use crate::compiler::NavProperty;
use crate::compiler::OData;
use crate::compiler::Parameter;
use crate::compiler::ParameterType;
use crate::compiler::Properties;
use crate::compiler::Property;
use crate::compiler::PropertyType;
use crate::compiler::QualifiedName;
use crate::compiler::TypeClass;
use crate::generator::rust::ActionName;
use crate::generator::rust::Config;
use crate::generator::rust::Error;
use crate::generator::rust::FullTypeName;
use crate::generator::rust::StructFieldName;
use crate::generator::rust::TypeName;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::ToTokens as _;
use quote::TokenStreamExt as _;
use quote::quote;

#[derive(Debug)]
pub enum GenerateType {
    Read,
    Update,
    Action,
}

/// Generation of Rust struct.
#[derive(Debug)]
pub struct StructDef<'a> {
    pub name: TypeName<'a>,
    base: Option<QualifiedName<'a>>,
    properties: Properties<'a>,
    parameters: Vec<Parameter<'a>>,
    actions: ActionsMap<'a>,
    odata: OData<'a>,
    generate: Vec<GenerateType>,
}

impl<'a> StructDef<'a> {
    /// Create `StructDef` builder.
    #[must_use]
    pub fn builder(name: TypeName<'a>, odata: OData<'a>) -> StructDefBuilder<'a> {
        StructDefBuilder::new(name, odata)
    }

    /// Generate rust code for the structure.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        for t in &self.generate {
            match t {
                GenerateType::Read => self.generate_read(tokens, config),
                GenerateType::Update => self.generate_update(tokens, config),
                GenerateType::Action => self.generate_action(tokens, config),
            }
        }
    }

    /// Generate rust code for the structure.
    pub fn generate_read(&self, tokens: &mut TokenStream, config: &Config) {
        enum ImplOdataType {
            Root,
            Child,
            None,
        }

        let top = &config.top_module_alias;
        let mut content = TokenStream::new();
        let odata_id = Ident::new("odata_id", Span::call_site());
        let odata_etag = Ident::new("odata_etag", Span::call_site());
        let impl_odata_type = if let Some(base) = self.base {
            let base_pname = StructFieldName::new_property(&config.base_type_prop_name);
            let typename = FullTypeName::new(base, config);
            content.extend(quote! {
                /// Base type
                #[serde(flatten)]
                pub #base_pname: #typename,
            });
            if *self.odata.must_have_id.inner() {
                ImplOdataType::Child
            } else {
                ImplOdataType::None
            }
        } else if *self.odata.must_have_id.inner() {
            // MustHaveId only for the root elements in type hierarchy. This requirements by code
            // generation. Generator needs to add @odata.id field to the struct.
            // If we will add odata.id on each level it may break deserialization.
            content.extend(quote! {
                #[serde(rename="@odata.id")]
                pub #odata_id: ODataId,
                #[serde(rename="@odata.etag")]
                pub #odata_etag: Option<ODataETag>,
            });
            ImplOdataType::Root
        } else {
            ImplOdataType::None
        };

        for p in &self.properties.properties {
            if p.odata.permissions_is_write_only() {
                continue;
            }
            Self::generate_property(&mut content, p, config);
        }

        for p in &self.properties.nav_properties {
            if p.odata.permissions_is_write_only() {
                continue;
            }
            Self::generate_nav_property(&mut content, p, config);
        }

        let mut actions = self.actions.values().collect::<Vec<_>>();
        actions.sort_by_key(|a| a.name);

        for a in &actions {
            Self::generate_action_property(&mut content, a, config);
        }

        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Deserialize, Debug)]
                pub struct #name { #content }
            },
        ]);

        match impl_odata_type {
            ImplOdataType::Root => {
                tokens.extend(quote! {
                    impl #top::EntityType for #name {
                        #[inline]
                        fn id(&self) -> &ODataId { &self.#odata_id }
                        #[inline]
                        fn etag(&self) -> &Option<ODataETag> { &self.#odata_etag }
                    }
                    impl #top::Expandable for #name {}
                });
            }
            ImplOdataType::Child => {
                tokens.extend(quote! {
                    impl #top::EntityType for #name {
                        #[inline]
                        fn id(&self) -> &ODataId { self.base.id() }
                        #[inline]
                        fn etag(&self) -> &Option<ODataETag> { self.base.etag() }
                    }
                    impl #top::Expandable for #name {}
                });
            }
            ImplOdataType::None => (),
        }

        if !actions.is_empty() {
            let mut content = TokenStream::new();
            for a in &actions {
                Self::generate_action_function(&mut content, a, config);
            }
            tokens.extend(quote! {
                impl #name
            });
            tokens.append(TokenTree::Group(Group::new(Delimiter::Brace, content)));
        }
    }

    /// Generate rust code for the update structure.
    pub fn generate_update(&self, tokens: &mut TokenStream, config: &Config) {
        let mut content = TokenStream::new();
        for p in &self.properties.properties {
            if !p.odata.permissions_is_write() {
                continue;
            }
            let rename = Literal::string(p.name.inner().inner());
            let name = StructFieldName::new_property(p.name);
            let (class, ptype @ (PropertyType::One(v) | PropertyType::CollectionOf(v))) = &p.ptype;
            let container = if matches!(ptype, PropertyType::One(_)) {
                Ident::new("Option", Span::call_site())
            } else {
                Ident::new("Vec", Span::call_site())
            };
            content.extend([
                quote! {
                    #[serde(rename=#rename)]
                    pub #name: #container
                },
                TokenTree::Punct(Punct::new('<', Spacing::Joint)).into(),
            ]);
            if *class == TypeClass::ComplexType {
                FullTypeName::new(*v, config)
                    .for_update()
                    .to_tokens(&mut content);
            } else {
                FullTypeName::new(*v, config).to_tokens(&mut content);
            }
            content.extend([
                TokenTree::Punct(Punct::new('>', Spacing::Joint)),
                TokenTree::Punct(Punct::new(',', Spacing::Alone)),
            ]);
        }
        let comment = format!(" Update struct corresponding to `{}`", self.name);
        let name = self.name.for_update();
        tokens.extend([quote! {
            #[doc = #comment]
            #[derive(Serialize, Debug)]
            pub struct #name
        }]);

        tokens.append(TokenTree::Group(Group::new(Delimiter::Brace, content)));
    }

    /// Generate Action struct.
    pub fn generate_action(&self, tokens: &mut TokenStream, config: &Config) {
        let mut content = TokenStream::new();
        for p in &self.parameters {
            Self::generate_action_parameter(&mut content, p, config);
        }

        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Serialize, Debug)]
                pub struct #name
            },
        ]);
        tokens.append(TokenTree::Group(Group::new(Delimiter::Brace, content)));
    }

    fn generate_property(content: &mut TokenStream, p: &Property<'_>, config: &Config) {
        content.extend(doc_format_and_generate(p.name, &p.odata));
        match p.ptype.1 {
            PropertyType::One(v) => {
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                let ptype = FullTypeName::new(v, config);
                content.extend(quote! { #[serde(rename=#rename)] });
                if p.redfish.is_required.into_inner() {
                    content.extend(quote! { pub #name: #ptype,  });
                } else {
                    content.extend(quote! { pub #name: Option<#ptype>, });
                }
            }
            PropertyType::CollectionOf(v) => {
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                let ptype = FullTypeName::new(v, config);
                if p.redfish.is_required.into_inner() {
                    content.extend(quote! { #[serde(rename=#rename)] });
                } else {
                    content.extend(quote! { #[serde(rename=#rename, default)] });
                }
                content.extend(quote! { pub #name: Vec<#ptype>, });
            }
        }
    }

    fn generate_nav_property(content: &mut TokenStream, p: &NavProperty<'_>, config: &Config) {
        content.extend(doc_format_and_generate(p.name, &p.odata));
        match p.ptype {
            PropertyType::One(v) => {
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                let ptype = FullTypeName::new(v, config);
                content.extend(quote! { #[serde(rename=#rename)] });
                if p.redfish.is_required.into_inner() {
                    content.extend(quote! { pub #name: NavProperty<#ptype>, });
                } else {
                    content.extend(quote! { pub #name: Option<NavProperty<#ptype>>, });
                }
            }
            PropertyType::CollectionOf(v) => {
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                let ptype = FullTypeName::new(v, config);
                if p.redfish.is_required.into_inner() {
                    content.extend(quote! { #[serde(rename=#rename)] });
                } else {
                    content.extend(quote! { #[serde(rename=#rename, default)] });
                }
                content.extend(quote! { pub #name: Vec<NavProperty<#ptype>>, });
            }
        }
    }

    fn generate_action_parameter(content: &mut TokenStream, p: &Parameter<'_>, config: &Config) {
        content.extend(doc_format_and_generate(p.name, &p.odata));
        match p.ptype {
            ParameterType::Type {
                ptype: ptype @ (PropertyType::One(v) | PropertyType::CollectionOf(v)),
                class,
            } => {
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_parameter(p.name);

                let mut base_type = TokenStream::new();
                if class == TypeClass::ComplexType {
                    FullTypeName::new(v, config)
                        .for_update()
                        .to_tokens(&mut base_type);
                } else {
                    FullTypeName::new(v, config).to_tokens(&mut base_type);
                }
                match ptype {
                    PropertyType::One(_) => {
                        if *p.is_nullable.inner() {
                            content.extend(quote! {
                                #[serde(rename=#rename)]
                                pub #name: #base_type,
                            });
                        } else {
                            content.extend(quote! {
                                #[serde(rename=#rename)]
                                pub #name: Option<#base_type>,
                            });
                        }
                    }
                    PropertyType::CollectionOf(_) => {
                        if *p.is_nullable.inner() {
                            content.extend(quote! {
                                #[serde(rename=#rename)]
                                pub #name: Vec<#base_type>,
                            });
                        } else {
                            content.extend(quote! {
                                #[serde(rename=#rename, default)]
                                pub #name: Vec<#base_type>,
                            });
                        }
                    }
                }
            }
            ParameterType::Entity(PropertyType::One(_)) => {
                let top = &config.top_module_alias;
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_parameter(p.name);
                content.extend(quote! { #[serde(rename=#rename)] });
                content.extend(quote! { pub #name: Option<#top::Reference>, });
            }
            ParameterType::Entity(PropertyType::CollectionOf(_)) => {
                let top = &config.top_module_alias;
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_parameter(p.name);
                content.extend(quote! { #[serde(rename=#rename, default)] });
                content.extend(quote! { pub #name: Vec<#top::Reference>, });
            }
        }
    }

    fn generate_action_property(content: &mut TokenStream, a: &Action, config: &Config) {
        let top = &config.top_module_alias;
        let rename = Literal::string(&format!("#{}.{}", a.binding_name, a.name));
        let name = ActionName::new(a.name);
        let typename = TypeName::new_action(a.binding_name, a.name);
        let mut ret_type = TokenStream::new();
        match a.return_type {
            Some(PropertyType::One(v)) => {
                FullTypeName::new(v, config).to_tokens(&mut ret_type);
            }
            Some(PropertyType::CollectionOf(v)) => {
                let mut typename = TokenStream::new();
                FullTypeName::new(v, config).to_tokens(&mut typename);
                ret_type.extend(quote! { Vec<#typename> });
            }
            None => ret_type.extend(quote! { () }),
        }
        content.extend(quote! { #[serde(rename=#rename)] });
        content.extend(quote! { pub #name: Option<#top::Action<#typename, #ret_type>>, });
    }

    fn generate_action_function(content: &mut TokenStream, a: &Action, config: &Config) {
        let top = &config.top_module_alias;
        let name = ActionName::new(a.name);
        let typename = TypeName::new_action(a.binding_name, a.name);
        let mut ret_type = TokenStream::new();
        match a.return_type {
            Some(PropertyType::One(v)) => {
                FullTypeName::new(v, config).to_tokens(&mut ret_type);
            }
            Some(PropertyType::CollectionOf(v)) => {
                let mut typename = TokenStream::new();
                FullTypeName::new(v, config).to_tokens(&mut typename);
                ret_type.extend(quote! { Vec<#typename> });
            }
            None => ret_type.extend(quote! { () }),
        }
        if a.parameters.len() <= config.action_fn_max_param_number_threshold {
            let mut arglist = TokenStream::new();
            let mut params = TokenStream::new();
            for p in &a.parameters {
                let top = &config.top_module_alias;
                let name = StructFieldName::new_parameter(p.name);
                params.extend(quote! { #name, });
                match p.ptype {
                    ParameterType::Type {
                        ptype: ptype @ (PropertyType::One(v) | PropertyType::CollectionOf(v)),
                        class,
                    } => {
                        let mut base_type = TokenStream::new();
                        if class == TypeClass::ComplexType {
                            FullTypeName::new(v, config)
                                .for_update()
                                .to_tokens(&mut base_type);
                        } else {
                            FullTypeName::new(v, config).to_tokens(&mut base_type);
                        }
                        match ptype {
                            PropertyType::One(_) => {
                                if *p.is_nullable.inner() {
                                    arglist.extend(quote! {, #name: #base_type });
                                } else {
                                    arglist.extend(quote! {, #name: Option<#base_type> });
                                }
                            }
                            PropertyType::CollectionOf(_) => {
                                arglist.extend(quote! {, #name: Vec<#base_type> });
                            }
                        }
                    }
                    ParameterType::Entity(PropertyType::One(_)) => {
                        arglist.extend(quote! {, #name: Option<#top::Reference> });
                    }
                    ParameterType::Entity(PropertyType::CollectionOf(_)) => {
                        arglist.extend(quote! {, #name: Vec<#top::Reference> });
                    }
                }
            }
            content.extend([
                doc_format_and_generate(a.name, &a.odata),
                quote! {
                    pub async fn #name<B: #top::Bmc>(&self, bmc: &B #arglist) -> Result<#ret_type, #top::Error> {
                        if let Some(a) = &self.#name  {
                            a.run(bmc, &#typename{
                                #params
                            }).await
                        } else {
                            Err(#top::Error::ActionIsNotSupported)
                        }
                    }
                },
            ]);
        } else {
            content.extend([
                doc_format_and_generate(a.name, &a.odata),
                quote! {
                    pub async fn #name<B: #top::Bmc>(&self, bmc: &B, t: &#typename) -> Result<#ret_type, #top::Error> {
                        if let Some(a) = &self.#name  {
                            a.run(bmc, t).await
                        } else {
                            Err(#top::Error::ActionIsNotSupported)
                        }
                    }
                },
            ]);
        }
    }
}

/// Builder of the `StructDef`
pub struct StructDefBuilder<'a>(StructDef<'a>);

impl<'a> StructDefBuilder<'a> {
    #[must_use]
    fn new(name: TypeName<'a>, odata: OData<'a>) -> Self {
        Self(StructDef {
            name,
            base: None,
            properties: Properties::default(),
            parameters: Vec::default(),
            actions: ActionsMap::default(),
            odata,
            generate: vec![GenerateType::Read],
        })
    }

    /// Setup base struct name for the struct.
    #[must_use]
    pub const fn with_base(mut self, base: QualifiedName<'a>) -> Self {
        self.0.base = Some(base);
        self
    }

    /// Setup action proprties for the struct.
    #[must_use]
    pub fn with_actions(mut self, actions: ActionsMap<'a>) -> Self {
        self.0.actions = actions;
        self
    }

    /// Setup structural and navigation proprties for the struct.
    #[must_use]
    pub fn with_properties(mut self, properties: Properties<'a>) -> Self {
        self.0.properties = properties;
        self
    }

    /// Setup parameters for the struct (for action structs).
    #[must_use]
    pub fn with_parameters(mut self, parameters: Vec<Parameter<'a>>) -> Self {
        self.0.parameters = parameters;
        self
    }

    /// Setup generation types for the struct.
    #[must_use]
    pub fn with_generate_type(mut self, generate: Vec<GenerateType>) -> Self {
        self.0.generate = generate;
        self
    }

    /// # Errors
    ///
    /// Returns error if struct definition cannot be generated by the
    /// provided parameters.
    pub fn build(self, config: &Config) -> Result<StructDef<'a>, Error<'a>> {
        if self.0.base.is_some() {
            let base_pname = StructFieldName::new_property(&config.base_type_prop_name);
            for p in &self.0.properties.properties {
                let pname = StructFieldName::new_property(p.name);
                if base_pname == pname {
                    return Err(Error::BaseTypeConflict);
                }
            }
            for p in &self.0.properties.nav_properties {
                let pname = StructFieldName::new_property(p.name);
                if base_pname == pname {
                    return Err(Error::BaseTypeConflict);
                }
            }
        }
        Ok(self.0)
    }
}
