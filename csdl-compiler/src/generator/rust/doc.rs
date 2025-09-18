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

//! Generation of Rust doc by comment lines.

use crate::compiler::CompiledOData;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use std::fmt::Display;

pub fn format_and_generate(name: impl Display, odata: &CompiledOData<'_>) -> TokenStream {
    format(name, odata)
        .map(|lines| generate(&lines))
        .unwrap_or_default()
}

#[must_use]
pub fn format(name: impl Display, odata: &CompiledOData<'_>) -> Option<Vec<String>> {
    let maybe_descr = odata.description.as_ref().map(ToString::to_string);
    let maybe_long_descr = odata.long_description.as_ref().map(ToString::to_string);
    match (maybe_descr, maybe_long_descr) {
        (None, None) => None,
        (Some(d), None) => Some(vec![format!(" {d}")]),
        (None, Some(ld)) => {
            let mut result = vec![format!(" {}", name), String::new()];
            result.extend(split_by_lines(&ld));
            Some(result)
        }
        (Some(d), Some(ld)) => {
            let mut result = split_by_lines(&d);
            result.push(String::new());
            result.extend(split_by_lines(&ld));
            Some(result)
        }
    }
}

#[must_use]
pub fn generate(lines: &[impl ToString]) -> TokenStream {
    let mut ts = TokenStream::new();
    for l in lines {
        let mut attr_inner = TokenStream::new();
        attr_inner.extend([
            TokenTree::Ident(Ident::new("doc", Span::call_site())),
            TokenTree::Punct(Punct::new('=', Spacing::Alone)),
            TokenTree::Literal(Literal::string(&l.to_string())),
        ]);
        ts.extend([
            TokenTree::Punct(Punct::new('#', Spacing::Alone)),
            TokenTree::Group(Group::new(Delimiter::Bracket, attr_inner)),
        ]);
    }
    ts
}

fn split_by_lines(s: &str) -> Vec<String> {
    s.split(' ')
        .fold(
            (Vec::<Vec<&str>>::new(), 0),
            |(mut lines, last_len), word| {
                if let Some(last) = lines.last_mut() {
                    if last_len + word.len() < 100 {
                        last.push(word);
                        return (lines, last_len + word.len() + 1);
                    }
                }
                lines.push(vec![word]);
                (lines, word.len() + 1)
            },
        )
        .0
        .into_iter()
        .map(|words| " ".to_owned() + &words.join(" "))
        .collect()
}
