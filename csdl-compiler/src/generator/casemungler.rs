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

#[must_use]
pub fn to_snake(s: impl AsRef<str>) -> String {
    tokenize_to_words(s.as_ref())
        .collect::<Vec<String>>()
        .join("_")
        .to_lowercase()
}

#[must_use]
pub fn to_camel(s: impl AsRef<str>) -> String {
    tokenize_to_words(s.as_ref()).fold(String::new(), |mut acc, word| {
        let mut itr = word.chars();
        if let Some(first) = itr.next() {
            acc.push(first.to_ascii_uppercase());
        }
        for ch in itr {
            acc.push(ch.to_ascii_lowercase());
        }
        acc
    })
}

fn tokenize_to_words(s: &str) -> impl Iterator<Item = String> {
    let chars: Vec<char> = s.chars().collect();

    chars
        .iter()
        .enumerate()
        .fold(vec![vec![]], |mut words: Vec<Vec<char>>, (i, &ch)| {
            // catch all situations where we need to separate stream of chars into words
            //if i > 0 && ch.is_uppercase() && {
            if ch == '_'
                || i > 0 && ch.is_uppercase() && {
                    let prev_char = chars[i - 1];

                    // case 1: transition from lower to uppercase (standard camelCase)
                    prev_char.is_lowercase() ||
                    // case 2: transition from acronym to a new word
                    (prev_char.is_uppercase() &&
                        i + 1 < chars.len() && chars[i + 1].is_lowercase() &&
                        // Count following lowercase letters to identify complete words
                        chars[(i + 1)..]
                            .iter()
                            .take_while(|&&c| c.is_lowercase())
                            .count() >= 2)
                }
            {
                words.push(vec![]);
            }

            if let Some(curr_word) = words.last_mut() {
                if ch != '_' {
                    curr_word.push(ch);
                }
            }
            words
        })
        .into_iter()
        .map(|w| w.into_iter().collect::<String>())
        .collect::<Vec<String>>()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_casemungler_as_string() {
        let owned_string = String::from("CamelCase");
        assert_eq!(to_snake(owned_string), "camel_case");

        let owned_string = String::from("Camel_Case");
        assert_eq!(to_camel(owned_string), "CamelCase");
    }

    #[test]
    fn test_casemungler_as_str() {
        let s = "CamelCase";
        assert_eq!(to_snake(s), "camel_case");

        let s = "Camel_Case";
        assert_eq!(to_camel(s), "CamelCase");
    }

    #[test]
    fn test_casemungler_to_snake() {
        assert_eq!(to_snake(""), "");
        assert_eq!(to_snake("_"), "_");
        assert_eq!(to_snake("___"), "___");
        assert_eq!(to_snake("F"), "f");
        assert_eq!(to_snake("PF"), "pf");
        assert_eq!(to_snake("pF"), "p_f");
        assert_eq!(to_snake("_SomeThing"), "_some_thing");
        assert_eq!(to_snake("_SomeBadMojo"), "_some_bad_mojo");
        assert_eq!(to_snake("_Some_Bad_Mojo"), "_some_bad_mojo");
        assert_eq!(to_snake("NVMe"), "nvme");
        assert_eq!(to_snake("NVME"), "nvme");
        assert_eq!(to_snake("nVME"), "n_vme");
        assert_eq!(to_snake("nVMEfoobar"), "n_vm_efoobar");
        assert_eq!(to_snake("nVMEFoobar"), "n_vme_foobar");
        assert_eq!(to_snake("PCIe_Functions"), "pcie_functions");
        assert_eq!(to_snake("PCIeFunctions"), "pcie_functions");
        assert_eq!(to_snake("PCIEFunctions"), "pcie_functions");
        assert_eq!(to_snake("PFFunctionNumber"), "pf_function_number");
        assert_eq!(to_snake("PhysFunctionNumber"), "phys_function_number");
        assert_eq!(to_snake("physFunctionNumber"), "phys_function_number");
        assert_eq!(to_snake("FOO_BAR"), "foo_bar");
        assert_eq!(to_snake("Foo_Bar"), "foo_bar");
        assert_eq!(to_snake("Foo_bar"), "foo_bar");
        assert_eq!(to_snake("FooBar"), "foo_bar");
        assert_eq!(to_snake("Foobar"), "foobar");
    }

    #[test]
    fn test_casemungler_to_camel() {
        assert_eq!(to_camel(""), "");
        assert_eq!(to_camel("_"), "");
        assert_eq!(to_camel("___"), "");
        assert_eq!(to_camel("F"), "F");
        assert_eq!(to_camel("PF"), "Pf");
        assert_eq!(to_camel("pF"), "PF");
        assert_eq!(to_camel("_SomeThing"), "SomeThing");
        assert_eq!(to_camel("_SomeThingIsNotRight"), "SomeThingIsNotRight");
        assert_eq!(to_camel("_Some_Thing_Is_Not_Right"), "SomeThingIsNotRight");
        assert_eq!(to_camel("NVMe"), "Nvme");
        assert_eq!(to_camel("NVME"), "Nvme");
        assert_eq!(to_camel("nVME"), "NVme");
        assert_eq!(to_camel("nVMEfoobar"), "NVmEfoobar");
        assert_eq!(to_camel("nVMEFoobar"), "NVmeFoobar");
        assert_eq!(to_camel("PCIe_Functions"), "PcieFunctions");
        assert_eq!(to_camel("PCIeFunctions"), "PcieFunctions");
        assert_eq!(to_camel("PCIEFunctions"), "PcieFunctions");
        assert_eq!(to_camel("PFFunctionNumber"), "PfFunctionNumber");
        assert_eq!(to_camel("PhysicalFunctionNumber"), "PhysicalFunctionNumber");
        assert_eq!(to_camel("physicalFunctionNumber"), "PhysicalFunctionNumber");
        assert_eq!(to_camel("FOO_BAR"), "FooBar");
        assert_eq!(to_camel("Foo_Bar"), "FooBar");
        assert_eq!(to_camel("Foo_bar"), "FooBar");
        assert_eq!(to_camel("FooBar"), "FooBar");
        assert_eq!(to_camel("Foobar"), "Foobar");
    }
}
