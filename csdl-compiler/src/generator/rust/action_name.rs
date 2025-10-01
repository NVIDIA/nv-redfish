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

use crate::compiler::Namespace;
use crate::edmx::ActionName as EdmxActionName;
use crate::edmx::ParameterName;
use crate::generator::casemungler;
use crate::generator::rust::Config;
use crate::generator::rust::ModName;
use crate::generator::rust::TypeName;
use proc_macro2::Ident;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use quote::TokenStreamExt as _;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Property name built from edmx `ActionName`.
///
/// Example of representation: `protocol_features_supported`
#[derive(PartialEq, Eq, Hash, Copy, Clone, Ord, PartialOrd)]
pub struct ActionName<'a>(&'a EdmxActionName);

impl<'a> ActionName<'a> {
    /// Create new property name.
    #[must_use]
    pub const fn new(v: &'a EdmxActionName) -> Self {
        Self(v)
    }
}

impl ToTokens for ActionName<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.to_string().as_str() {
            "type" => tokens.append(Ident::new_raw("type", Span::call_site())),
            _ => tokens.append(Ident::new(&self.to_string(), Span::call_site())),
        }
    }
}

impl Display for ActionName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&casemungler::to_snake(self.0.inner()))
    }
}

impl Debug for ActionName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

/// Fully quailified type name for generation of the rust code.
///
/// Example:
///
/// `redfish::computer_system::ComputerSystemResetAction`
pub struct ActionFullTypeName<'a, 'config> {
    binding_ns: Namespace<'a>,
    binding_name: &'a ParameterName,
    action_name: &'a EdmxActionName,
    config: &'config Config,
}

impl<'a, 'config> ActionFullTypeName<'a, 'config> {
    /// Create new fully qualified action name type.
    #[must_use]
    pub const fn new(
        binding_ns: Namespace<'a>,
        binding_name: &'a ParameterName,
        action_name: &'a EdmxActionName,
        config: &'config Config,
    ) -> Self {
        Self {
            binding_ns,
            binding_name,
            action_name,
            config,
        }
    }
}

impl ToTokens for ActionFullTypeName<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let top = &self.config.top_module_alias;
        tokens.extend(quote! { #top });
        for depth in 0..self.binding_ns.len() {
            if let Some(id) = self.binding_ns.get_id(depth) {
                let name = ModName::new(id);
                tokens.append(Punct::new(':', Spacing::Joint));
                tokens.append(Punct::new(':', Spacing::Joint));
                tokens.extend(quote! { #name });
            }
        }
        let name = TypeName::new_action(self.binding_name, self.action_name);
        tokens.append(Punct::new(':', Spacing::Joint));
        tokens.append(Punct::new(':', Spacing::Joint));
        tokens.extend(quote! { #name });
    }
}
