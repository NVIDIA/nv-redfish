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

use crate::OneOrCollection;
use crate::compiler::Action;
use crate::compiler::ActionsMap;
use crate::compiler::NavProperty;
use crate::compiler::NavPropertyType;
use crate::compiler::OData;
use crate::compiler::Parameter;
use crate::compiler::ParameterType;
use crate::compiler::Properties;
use crate::compiler::Property;
use crate::compiler::PropertyType;
use crate::compiler::QualifiedName;
use crate::compiler::TypeClass;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use crate::generator::rust::ActionName;
use crate::generator::rust::Config;
use crate::generator::rust::Error;
use crate::generator::rust::FullTypeName;
use crate::generator::rust::StructFieldName;
use crate::generator::rust::TypeName;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens as _;

#[derive(Debug)]
pub enum GenerateType {
    Read,
    Update,
    Create,
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
    create_type: Option<QualifiedName<'a>>,
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
                GenerateType::Create => self.generate_create(tokens, config),
                GenerateType::Read => self.generate_read(tokens, config),
                GenerateType::Update => self.generate_update(tokens, config),
                GenerateType::Action => self.generate_action(tokens, config),
            }
        }
    }

    /// Generate rust code for the structure.
    pub fn generate_read(&self, tokens: &mut TokenStream, config: &Config) {
        #[derive(PartialEq, Eq)]
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
            Self::generate_nav_property(&mut content, p, config);
        }

        let mut actions = self.actions.values().collect::<Vec<_>>();
        actions.sort_by_key(|a| a.name);

        for a in &actions {
            Self::generate_action_property(&mut content, a, config);
        }

        if self.odata.additional_properties.is_some_and(|v| *v.inner()) {
            // If additional_properties are explicitly set then we add
            // placeholder with serde_json::Value to
            // deserializer. Actually, it is almost always Oem /
            // OemAction.
            content.extend(quote! {
                #[serde(flatten)]
                pub additional_properties: #top::AdditionalProperties,
            });
        }

        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Deserialize, Debug)]
                pub struct #name { #content }
            },
        ]);

        let entity_type_impl = |fn_id_impl, fn_etag_impl| {
            quote! {
                impl #top::EntityType for #name {
                    #[inline] fn id(&self) -> &ODataId { #fn_id_impl }
                    #[inline] fn etag(&self) -> &Option<ODataETag> { #fn_etag_impl }
                }
            }
        };

        match impl_odata_type {
            ImplOdataType::Root => {
                tokens.extend(entity_type_impl(
                    quote! { &self.#odata_id },
                    quote! { &self.#odata_etag },
                ));
            }
            ImplOdataType::Child => {
                tokens.extend(entity_type_impl(
                    quote! { self.base.id() },
                    quote! { self.base.etag() },
                ));
            }
            ImplOdataType::None => (),
        }

        if impl_odata_type != ImplOdataType::None {
            self.generate_entity_type_traits(tokens, config);
        }

        if !actions.is_empty() {
            let mut content = TokenStream::new();
            for a in &actions {
                Self::generate_action_function(&mut content, a, config);
            }
            tokens.extend(quote! {
                impl #name { #content }
            });
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
            content.extend(quote! {
                #[serde(rename=#rename)]
            });

            let (class, v) = &p.ptype.inner();
            let mut full_type_name_tokens = TokenStream::new();
            if *class == TypeClass::ComplexType {
                FullTypeName::new(*v, config)
                    .for_update()
                    .to_tokens(&mut full_type_name_tokens);
            } else {
                FullTypeName::new(*v, config).to_tokens(&mut full_type_name_tokens);
            }
            match p.ptype {
                PropertyType::One(_) => content.extend(quote! {
                    pub #name: Option<#full_type_name_tokens>,
                }),
                PropertyType::Collection(_) => content.extend(quote! {
                    pub #name: Vec<#full_type_name_tokens>,
                }),
            }
        }
        let comment = format!(" Update struct corresponding to `{}`", self.name);
        let name = self.name.for_update();
        tokens.extend([quote! {
            #[doc = #comment]
            #[derive(Serialize, Debug, Default)]
            pub struct #name { #content }
        }]);
    }

    /// Generate rust code for the create structure.
    pub fn generate_create(&self, tokens: &mut TokenStream, config: &Config) {
        let mut content = TokenStream::new();
        for p in &self.properties.properties {
            if !p.odata.permissions_is_write() {
                continue;
            }

            let rename = Literal::string(p.name.inner().inner());
            let name = StructFieldName::new_property(p.name);
            let (class, v) = &p.ptype.inner();

            let mut full_type_name_tokens = TokenStream::new();
            if *class == TypeClass::ComplexType {
                FullTypeName::new(*v, config)
                    .for_update()
                    .to_tokens(&mut full_type_name_tokens);
            } else {
                FullTypeName::new(*v, config).to_tokens(&mut full_type_name_tokens);
            }

            content.extend(quote! { #[serde(rename=#rename)] });
            if p.redfish.is_required_on_create.into_inner() {
                match p.ptype {
                    PropertyType::One(_) => {
                        content.extend(quote! { pub #name: #full_type_name_tokens, });
                    }
                    PropertyType::Collection(_) => {
                        content.extend(quote! { pub #name: Vec<#full_type_name_tokens>, });
                    }
                }
            } else {
                match p.ptype {
                    PropertyType::One(_) => {
                        content.extend(quote! { pub #name: Option<#full_type_name_tokens>, });
                    }
                    PropertyType::Collection(_) => {
                        content.extend(quote! { pub #name: Vec<#full_type_name_tokens>, });
                    }
                }
            }
        }
        let comment = format!(" Create struct corresponding to `{}`", self.name);
        let name = self.name.for_create();
        tokens.extend([quote! {
            #[doc = #comment]
            #[derive(Serialize, Debug, Default)]
            pub struct #name { #content }
        }]);
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
                pub struct #name { #content }
            },
        ]);
    }

    fn generate_property(content: &mut TokenStream, p: &Property<'_>, config: &Config) {
        content.extend(doc_format_and_generate(p.name, &p.odata));
        let name = StructFieldName::new_property(p.name);
        let rename = Literal::string(p.name.inner().inner());
        let ptype = FullTypeName::new(p.ptype.name(), config);
        match p.ptype {
            PropertyType::One(_) => {
                content.extend(quote! { #[serde(rename=#rename)] });
                if p.redfish.is_required.into_inner() {
                    content.extend(quote! { pub #name: #ptype,  });
                } else {
                    content.extend(quote! { pub #name: Option<#ptype>, });
                }
            }
            PropertyType::Collection(_) => {
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
        match p {
            NavProperty::Expandable(p) => {
                if p.odata.permissions_is_write_only() {
                    return;
                }
                content.extend(doc_format_and_generate(p.name, &p.odata));
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                let ptype = FullTypeName::new(p.ptype.name(), config);
                match p.ptype {
                    NavPropertyType::One(_) => {
                        content.extend(quote! { #[serde(rename=#rename)] });
                        if p.redfish.is_required.into_inner() {
                            content.extend(quote! { pub #name: NavProperty<#ptype>, });
                        } else {
                            content.extend(quote! { pub #name: Option<NavProperty<#ptype>>, });
                        }
                    }
                    NavPropertyType::Collection(_) => {
                        if p.redfish.is_required.into_inner() {
                            content.extend(quote! { #[serde(rename=#rename)] });
                        } else {
                            content.extend(quote! { #[serde(rename=#rename, default)] });
                        }
                        content.extend(quote! { pub #name: Vec<NavProperty<#ptype>>, });
                    }
                }
            }
            NavProperty::Reference(OneOrCollection::One(name)) => {
                let top = &config.top_module_alias;
                let rename = Literal::string(name.inner().inner());
                let name = StructFieldName::new_property(name);
                content.extend(quote! {
                    #[serde(rename=#rename, default)]
                    pub #name: Option<#top::Reference>,
                });
            }
            NavProperty::Reference(OneOrCollection::Collection(name)) => {
                let top = &config.top_module_alias;
                let rename = Literal::string(name.inner().inner());
                let name = StructFieldName::new_property(name);
                content.extend(quote! {
                    #[serde(rename=#rename, default)]
                    pub #name: Vec<#top::Reference>,
                });
            }
        }
    }

    fn generate_action_parameter(content: &mut TokenStream, p: &Parameter<'_>, config: &Config) {
        content.extend(doc_format_and_generate(p.name, &p.odata));
        let rename = Literal::string(p.name.inner().inner());
        let name = StructFieldName::new_parameter(p.name);
        match p.ptype {
            ParameterType::Type(
                ptype @ (PropertyType::One((class, v)) | PropertyType::Collection((class, v))),
            ) => {
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
                    PropertyType::Collection(_) => {
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
            ParameterType::Entity(NavPropertyType::One(_)) => {
                let top = &config.top_module_alias;
                content.extend(quote! { #[serde(rename=#rename)] });
                content.extend(quote! { pub #name: Option<#top::Reference>, });
            }
            ParameterType::Entity(NavPropertyType::Collection(_)) => {
                let top = &config.top_module_alias;
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
            Some(OneOrCollection::One(v)) => {
                FullTypeName::new(v, config).to_tokens(&mut ret_type);
            }
            Some(OneOrCollection::Collection(v)) => {
                let mut typename = TokenStream::new();
                FullTypeName::new(v, config).to_tokens(&mut typename);
                ret_type.extend(quote! { Vec<#typename> });
            }
            None => ret_type.extend(quote! { #top::Empty }),
        }
        content.extend(quote! { #[serde(rename=#rename)] });
        content.extend(quote! { pub #name: Option<#top::Action<#typename, #ret_type>>, });
    }

    fn generate_entity_type_traits(&self, tokens: &mut TokenStream, config: &Config) {
        let name = self.name;
        let top = &config.top_module_alias;
        tokens.extend(quote! {
            impl #top::Expandable for #name {}
        });

        if self.odata.updatable.is_some_and(|v| v.inner().value) {
            let update_name = self.name.for_update();
            tokens.extend(quote! {
                impl #top::Updatable<#update_name> for #name {}
            });
        }

        if self.odata.deletable.is_some_and(|v| v.inner().value) {
            tokens.extend(quote! {
                impl #top::Deletable for #name {}
            });
        }

        if let Some(create_type) = self.create_type {
            let result_name = FullTypeName::new(create_type, config);
            let create_name = result_name.for_create();
            tokens.extend(quote! {
                impl #top::Creatable<#create_name, #result_name> for #name {}
            });
        }
    }

    fn generate_action_function(content: &mut TokenStream, a: &Action, config: &Config) {
        let top = &config.top_module_alias;
        let name = ActionName::new(a.name);
        let typename = TypeName::new_action(a.binding_name, a.name);
        let mut ret_type = TokenStream::new();
        match a.return_type {
            Some(OneOrCollection::One(v)) => {
                FullTypeName::new(v, config).to_tokens(&mut ret_type);
            }
            Some(OneOrCollection::Collection(v)) => {
                let mut typename = TokenStream::new();
                FullTypeName::new(v, config).to_tokens(&mut typename);
                ret_type.extend(quote! { Vec<#typename> });
            }
            None => ret_type.extend(quote! { #top::Empty }),
        }
        if a.parameters.len() <= config.action_fn_max_param_number_threshold {
            let mut arglist = TokenStream::new();
            let mut params = TokenStream::new();
            for p in &a.parameters {
                let top = &config.top_module_alias;
                let name = StructFieldName::new_parameter(p.name);
                params.extend(quote! { #name, });
                match p.ptype {
                    ParameterType::Type(
                        ptype @ (PropertyType::One((class, v))
                        | PropertyType::Collection((class, v))),
                    ) => {
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
                            PropertyType::Collection(_) => {
                                arglist.extend(quote! {, #name: Vec<#base_type> });
                            }
                        }
                    }
                    ParameterType::Entity(NavPropertyType::One(_)) => {
                        arglist.extend(quote! {, #name: Option<#top::Reference> });
                    }
                    ParameterType::Entity(NavPropertyType::Collection(_)) => {
                        arglist.extend(quote! {, #name: Vec<#top::Reference> });
                    }
                }
            }
            content.extend([
                doc_format_and_generate(a.name, &a.odata),
                quote! {
                    pub async fn #name<B: #top::Bmc>(&self, bmc: &B #arglist) -> Result<#ret_type, B::Error>
                    where B::Error: #top::ActionError,
                    {
                        if let Some(a) = &self.#name  {
                            a.run(bmc, &#typename{
                                #params
                            }).await
                        } else {
                            Err(B::Error::not_supported())
                        }
                    }
                },
            ]);
        } else {
            content.extend([
                doc_format_and_generate(a.name, &a.odata),
                quote! {
                    pub async fn #name<B: #top::Bmc>(&self, bmc: &B, t: &#typename) -> Result<#ret_type, B::Error>
                    where B::Error: #top::ActionError,
                    {
                        if let Some(a) = &self.#name  {
                            a.run(bmc, t).await
                        } else {
                            Err(B::Error::not_supported())
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
            create_type: None,
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

    /// Setup create type for the struct.
    #[must_use]
    pub const fn with_create(mut self, ct: QualifiedName<'a>) -> Self {
        self.0.create_type = Some(ct);
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
                let pname = StructFieldName::new_property(p.name());
                if base_pname == pname {
                    return Err(Error::BaseTypeConflict);
                }
            }
        }
        Ok(self.0)
    }
}
