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
use crate::compiler::OData;
use crate::compiler::Parameter;
use crate::compiler::ParameterType;
use crate::compiler::Properties;
use crate::compiler::Property;
use crate::compiler::PropertyType;
use crate::compiler::QualifiedName;
use crate::generator::rust::ActionName;
use crate::generator::rust::Config;
use crate::generator::rust::Error;
use crate::generator::rust::FullTypeName;
use crate::generator::rust::StructFieldName;
use crate::generator::rust::TypeName;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use crate::odata::annotations::Permissions;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::quote;
use std::iter;

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

#[derive(PartialEq, Eq)]
enum ImplOdataType {
    Root,
    Child,
    None,
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

    fn generate_read(&self, tokens: &mut TokenStream, config: &Config) {
        let top = &config.top_module_alias;
        let mut content = TokenStream::new();
        let odata_id = Ident::new("odata_id", Span::call_site());
        let odata_etag = Ident::new("odata_etag", Span::call_site());
        let (base_props, impl_odata_type) = self.base_type(&odata_id, &odata_etag, config);

        // Properties token streams:
        let properties_iter = self.properties.properties.iter().filter_map(|p| {
            if p.odata.permissions_is_write_only() {
                None
            } else {
                Some(Self::generate_property(p, config))
            }
        });

        // Navigation properties token streams:
        let nav_properties_iter = self
            .properties
            .nav_properties
            .iter()
            .map(|p| Self::generate_nav_property(p, config));

        // Action properties token streams:
        let mut actions = self.actions.values().collect::<Vec<_>>();
        actions.sort_by_key(|a| a.name);
        let action_iter = actions
            .iter()
            .map(|a| Self::generate_action_property(a, config));

        let additional_properties = if self.odata.additional_properties.is_some_and(|v| *v.inner())
        {
            // If additional_properties are explicitly set then we add
            // placeholder with serde_json::Value to
            // deserializer. Actually, it is almost always Oem /
            // OemAction.
            quote! {
                #[serde(flatten)]
                pub additional_properties: #top::AdditionalProperties,
            }
        } else {
            TokenStream::new()
        };

        // Combine all together in content
        let all_properties = iter::once(base_props)
            .chain(properties_iter)
            .chain(nav_properties_iter)
            .chain(action_iter)
            .chain(iter::once(additional_properties));

        content.extend(all_properties);

        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Deserialize, Debug)]
                pub struct #name { #content }
            },
        ]);

        // Additional function that are implemented for type:
        let entity_type_impl = |fn_id_impl, fn_etag_impl| {
            quote! {
                impl #top::EntityType for #name {
                    #[inline] fn id(&self) -> &ODataId { #fn_id_impl }
                    #[inline] fn etag(&self) -> &Option<ODataETag> { #fn_etag_impl }
                }
            }
        };

        tokens.extend(match impl_odata_type {
            ImplOdataType::Root => {
                entity_type_impl(quote! { &self.#odata_id }, quote! { &self.#odata_etag })
            }
            ImplOdataType::Child => {
                entity_type_impl(quote! { self.base.id() }, quote! { self.base.etag() })
            }
            ImplOdataType::None => TokenStream::new(),
        });

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

    fn base_type(
        &self,
        odata_id: &Ident,
        odata_etag: &Ident,
        config: &Config,
    ) -> (TokenStream, ImplOdataType) {
        self.base.map_or_else(
            || {
                if *self.odata.must_have_id.inner() {
                    // MustHaveId only for the root elements in type hierarchy. This requirements by code
                    // generation. Generator needs to add @odata.id field to the struct.
                    // If we will add odata.id on each level it may break deserialization.
                    (
                        quote! {
                            #[serde(rename="@odata.id")]
                            pub #odata_id: ODataId,
                            #[serde(rename="@odata.etag")]
                            pub #odata_etag: Option<ODataETag>,
                        },
                        ImplOdataType::Root,
                    )
                } else {
                    (TokenStream::new(), ImplOdataType::None)
                }
            },
            |base| {
                let base_pname = StructFieldName::new_property(&config.base_type_prop_name);
                let typename = FullTypeName::new(base, config);
                (
                    quote! {
                        /// Base type
                        #[serde(flatten)]
                        pub #base_pname: #typename,
                    },
                    if *self.odata.must_have_id.inner() {
                        ImplOdataType::Child
                    } else {
                        ImplOdataType::None
                    },
                )
            },
        )
    }

    fn generate_update(&self, tokens: &mut TokenStream, config: &Config) {
        let properties = self.properties.properties.iter().filter_map(|p| {
            let (typeinfo, v) = &p.ptype.inner();
            if p.odata.permissions_is_write()
                && typeinfo.permissions.is_none_or(|p| p != Permissions::Read)
            {
                let full_type = FullTypeName::new(*v, config).for_update(Some(typeinfo.class));
                let prop_type = match p.ptype {
                    PropertyType::One(_) => quote! { Option<#full_type> },
                    PropertyType::Collection(_) => quote! { Option<Vec<#full_type>> },
                };
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                Some(quote! {
                    #[serde(rename=#rename)]
                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub #name: #prop_type,
                })
            } else {
                None
            }
        });
        let mut content = TokenStream::new();
        content.extend(properties);
        let comment = format!(" Update struct corresponding to `{}`", self.name);
        let name = self.name.for_update(None);
        tokens.extend([quote! {
            #[doc = #comment]
            #[derive(Serialize, Debug, Default)]
            pub struct #name { #content }
        }]);
    }

    fn generate_create(&self, tokens: &mut TokenStream, config: &Config) {
        let properties = self.properties.properties.iter().filter_map(|p| {
            let (typeinfo, v) = &p.ptype.inner();
            if p.odata.permissions_is_write()
                && typeinfo.permissions.is_none_or(|p| p != Permissions::Read)
            {
                let full_type = FullTypeName::new(*v, config).for_update(Some(typeinfo.class));
                let (serde_opt, prop_type) = match p.ptype {
                    PropertyType::One(_) => {
                        if p.redfish.is_required_on_create.into_inner() {
                            (quote! {}, quote! { #full_type })
                        } else {
                            (
                                quote! {#[serde(skip_serializing_if = "Option::is_none")]},
                                quote! { Option<#full_type> },
                            )
                        }
                    }
                    PropertyType::Collection(_) => {
                        if p.redfish.is_required_on_create.into_inner() {
                            (quote! {}, quote! { Vec<#full_type> })
                        } else {
                            (
                                quote! {#[serde(skip_serializing_if = "Option::is_none")]},
                                quote! { Option<Vec<#full_type>> },
                            )
                        }
                    }
                };
                let rename = Literal::string(p.name.inner().inner());
                let name = StructFieldName::new_property(p.name);
                Some(quote! {
                    #[serde(rename=#rename)]
                    #serde_opt
                    pub #name: #prop_type,
                })
            } else {
                None
            }
        });

        let mut content = TokenStream::new();
        content.extend(properties);
        let comment = format!(" Create struct corresponding to `{}`", self.name);
        let name = self.name.for_create();
        tokens.extend([quote! {
            #[doc = #comment]
            #[derive(Serialize, Debug, Default)]
            pub struct #name { #content }
        }]);
    }

    fn generate_action(&self, tokens: &mut TokenStream, config: &Config) {
        let mut content = TokenStream::new();
        content.extend(
            self.parameters
                .iter()
                .map(|p| Self::generate_action_parameter(p, config)),
        );

        let name = self.name;
        tokens.extend([
            doc_format_and_generate(self.name, &self.odata),
            quote! {
                #[derive(Serialize, Debug)]
                pub struct #name { #content }
            },
        ]);
    }

    fn generate_property(p: &Property<'_>, config: &Config) -> TokenStream {
        let doc = doc_format_and_generate(p.name, &p.odata);
        let (serde, field_type) = Self::gen_de_struct_field(
            &p.ptype,
            FullTypeName::new(p.ptype.name(), config),
            Literal::string(p.name.inner().inner()),
            Some(!p.redfish.is_required.into_inner()),
        );
        let name = StructFieldName::new_property(p.name);
        quote! {
            #doc #serde
            pub #name: #field_type,
        }
    }

    // Returns serde annotation and field type token streams.
    fn gen_de_struct_field<T>(
        cardinality: &OneOrCollection<T>,
        ftype: impl ToTokens,
        rename: impl ToTokens,
        optional: Option<bool>,
    ) -> (TokenStream, TokenStream) {
        (
            Self::gen_de_struct_field_serde_annot(cardinality, rename, optional),
            Self::gen_de_struct_field_type(cardinality, ftype, optional),
        )
    }

    fn gen_de_struct_field_serde_annot<T>(
        cardinality: &OneOrCollection<T>,
        rename: impl ToTokens,
        optional: Option<bool>,
    ) -> TokenStream {
        match cardinality {
            OneOrCollection::One(_) => quote! { #[serde(rename=#rename)] },
            OneOrCollection::Collection(_) => {
                if optional.is_none_or(|v| v) {
                    quote! { #[serde(rename=#rename, default)] }
                } else {
                    quote! { #[serde(rename=#rename)] }
                }
            }
        }
    }

    // Returns serde annotation and field type token streams.
    fn gen_de_struct_field_type<T>(
        cardinality: &OneOrCollection<T>,
        ftype: impl ToTokens,
        optional: Option<bool>,
    ) -> TokenStream {
        match cardinality {
            OneOrCollection::One(_) => {
                if optional.is_none_or(|v| v) {
                    quote! { Option<#ftype> }
                } else {
                    quote! { #ftype }
                }
            }
            OneOrCollection::Collection(_) => quote! { Vec<#ftype> },
        }
    }

    fn generate_nav_property(p: &NavProperty<'_>, config: &Config) -> TokenStream {
        let name = StructFieldName::new_property(p.name());
        let rename = Literal::string(p.name().inner().inner());
        let (doc, serde, prop_type) = match p {
            NavProperty::Expandable(p) => {
                if p.odata.permissions_is_write_only() {
                    return TokenStream::new();
                }
                let doc = doc_format_and_generate(p.ptype.name(), &p.odata);
                let full_type = FullTypeName::new(p.ptype.name(), config);
                let optional = Some(!p.redfish.is_required.into_inner());
                let ptype = quote! { NavProperty<#full_type> };
                let (sa, t) = Self::gen_de_struct_field(&p.ptype, ptype, rename, optional);
                (doc, sa, t)
            }
            NavProperty::Reference(r) => {
                let doc = TokenStream::new();
                let top = &config.top_module_alias;
                let optional = None;
                let ptype = quote! { #top::Reference };
                let (sa, t) = Self::gen_de_struct_field(r, ptype, rename, optional);
                (doc, sa, t)
            }
        };
        quote! {
            #doc
            #serde
            pub #name: #prop_type,
        }
    }

    fn generate_action_parameter(p: &Parameter<'_>, config: &Config) -> TokenStream {
        let doc = doc_format_and_generate(p.name, &p.odata);
        let rename = Literal::string(p.name.inner().inner());
        let name = StructFieldName::new_parameter(p.name);
        let (serde, ptype) = match p.ptype {
            ParameterType::Type(
                ptype
                @ (PropertyType::One((typeinfo, v)) | PropertyType::Collection((typeinfo, v))),
            ) => {
                let full_type = FullTypeName::new(v, config).for_update(Some(typeinfo.class));
                let optional = Some(*p.is_nullable.inner());
                Self::gen_de_struct_field(&ptype, full_type, rename, optional)
            }
            ParameterType::Entity(e) => {
                let top = &config.top_module_alias;
                Self::gen_de_struct_field(&e, quote! { #top::Reference }, rename, Some(true))
            }
        };
        quote! {
            #doc
            #serde
            pub #name: #ptype,
        }
    }

    fn generate_action_property(a: &Action, config: &Config) -> TokenStream {
        let top = &config.top_module_alias;
        let rename = Literal::string(&format!("#{}.{}", a.binding_name, a.name));
        let name = ActionName::new(a.name);
        let typename = TypeName::new_action(a.binding_name, a.name);
        let ret_type = match a.return_type {
            Some(OneOrCollection::One(v)) => FullTypeName::new(v, config).to_token_stream(),
            Some(OneOrCollection::Collection(v)) => {
                let typename = FullTypeName::new(v, config);
                quote! { Vec<#typename> }
            }
            None => quote! { #top::Empty },
        };
        quote! {
            #[serde(rename=#rename)]
            pub #name: Option<#top::Action<#typename, #ret_type>>,
        }
    }

    fn generate_entity_type_traits(&self, tokens: &mut TokenStream, config: &Config) {
        let name = self.name;
        let top = &config.top_module_alias;
        tokens.extend(quote! {
            impl #top::Expandable for #name {}
        });

        if self.odata.updatable.is_some_and(|v| v.inner().value) {
            let update_name = self.name.for_update(None);
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
        let ret_type = match a.return_type {
            Some(OneOrCollection::One(v)) => FullTypeName::new(v, config).to_token_stream(),
            Some(OneOrCollection::Collection(v)) => {
                let typename = FullTypeName::new(v, config);
                quote! { Vec<#typename> }
            }
            None => quote! { #top::Empty },
        };
        if a.parameters.len() <= config.action_fn_max_param_number_threshold {
            let mut arglist = TokenStream::new();
            let mut params = TokenStream::new();
            for p in &a.parameters {
                let top = &config.top_module_alias;
                let name = StructFieldName::new_parameter(p.name);
                params.extend(quote! { #name, });
                let argtype = match p.ptype {
                    ParameterType::Type(
                        ptype @ (PropertyType::One((typeinfo, v))
                        | PropertyType::Collection((typeinfo, v))),
                    ) => {
                        let full_type =
                            FullTypeName::new(v, config).for_update(Some(typeinfo.class));
                        Self::gen_de_struct_field_type(
                            &ptype,
                            full_type,
                            Some(*p.is_nullable.inner()),
                        )
                    }
                    ParameterType::Entity(e) => {
                        let full_type = quote! { #top::Reference };
                        Self::gen_de_struct_field_type(&e, full_type, Some(true))
                    }
                };
                arglist.extend(quote! {, #name: #argtype });
            }
            content.extend([
                doc_format_and_generate(a.name, &a.odata),
                quote! {
                    pub async fn #name<B: #top::Bmc>(&self, bmc: &B #arglist) -> Result<#ret_type, B::Error>
                    where B::Error: #top::ActionError,
                    {
                        if let Some(a) = &self.#name  {
                            a.run(bmc, &#typename {
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
