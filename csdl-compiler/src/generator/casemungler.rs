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

const SNAKE_WORD_SEPARATOR: &str = "~!#%^&*()+-:<>?,./ ";
const CAMEL_WORD_SEPARATOR: &str = "_~!#%^&*()+-:<>?,./ ";

/// Wrapper for snakecase producer 
#[must_use]
pub fn to_snake(s: impl AsRef<str>) -> String {
    tokenize(s.as_ref(), SNAKE_WORD_SEPARATOR).map_or_else(
        || String::from(s.as_ref()),
        |iter| iter.collect::<Vec<String>>().join("_").to_lowercase(),
    )
}

/// Wrapper for camelcase producer 
#[must_use]
pub fn to_camel(s: impl AsRef<str>) -> String {
    let orig_str = String::from(s.as_ref());

    if orig_str.len() < 2 {
        return orig_str;
    }

    tokenize(s.as_ref(), CAMEL_WORD_SEPARATOR).map_or(orig_str, |iter| {
        iter.fold(String::new(), |mut acc, word| {
            let mut word_iter = word.chars();
            if let Some(first) = word_iter.next() {
                acc.push(first.to_ascii_uppercase());
            }
            for ch in word_iter {
                acc.push(ch.to_ascii_lowercase());
            }
            acc
        })
    })
}

/// Tokenizer is a wrapper for word splitter with custom separators 
fn tokenize(s: &str, separators: &str) -> Option<impl Iterator<Item = String>> {
    let mut itr = split_to_words(s, separators).peekable();

    // NB: we are not in 2024 yet, so, no let-chains ;)
    if let Some(word) = itr.peek() {
        if !word.is_empty() {
            return Some(itr);
        }
    }
    None
}

/// A feeble attempt to determine words boundaries for camel and snake cases
fn is_word_boundary(chars: &[char], idx: usize, ch: char, separators: &str) -> bool {
    if separators.contains(ch) {
        return true;
    }

    // Don't split on first character
    if idx == 0 {
        return false;
    }

    // Only consider uppercase letters for camelCase splitting
    if !ch.is_uppercase() {
        return false;
    }

    let prev_char = chars[idx - 1];

    // Transition from lowercase to uppercase (standard camelCase: someWord)
    if prev_char.is_lowercase() {
        return true;
    }

    // Transitions between normal word and acronym
    is_acronym_to_word_transition(chars, idx)
}

/// Check if a chars slice index indicates a needed transition from acronym to a new word
fn is_acronym_to_word_transition(chars: &[char], idx: usize) -> bool {
    let prev_char = chars[idx - 1];

    // Previous character must be uppercase (part of acronym)
    if !prev_char.is_uppercase() {
        return false;
    }

    // Need at least one more char after current position
    if idx + 1 >= chars.len() {
        return false;
    }

    // Next character should be lowercase (start of a new word for camelcase)
    if !chars[idx + 1].is_lowercase() {
        return false;
    }

    // Count following lowercase letters to ensure it's a complete word (not just a single char)
    let lowercase_count = chars[(idx + 1)..]
        .iter()
        .take_while(|&&c| c.is_lowercase())
        .count();

    lowercase_count >= 2
}

/// Split a string slice into vector of strings (words) iterator so the caller
/// can do something with the resulting words 
fn split_to_words(s: &str, separators: &str) -> impl Iterator<Item = String> {
    let str_chars: Vec<char> = s.chars().collect();

    str_chars
        .iter()
        .enumerate()
        .fold(vec![vec![]], |mut words: Vec<Vec<char>>, (i, &ch)| {
            if is_word_boundary(&str_chars, i, ch, separators) {
                // Create a new word _only_ if the current word has 1+ character,
                // otherwise all weird corner cases will pop up 
                if words[words.len() - 1].len() > 1 {
                    words.push(vec![]);
                }
            }
            // Accumulate chars to the current word, don't keep the separator
            if !separators.contains(ch) {
                if let Some(curr_word) = words.last_mut() {
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

    // Common test patterns: (input, expected_snake, expected_camel)
    const TEST_PATTERNS: &[(&str, &str, &str)] = &[
        // Empty and separator cases
        ("", "", ""),
        ("_", "_", "_"),
        ("___", "___", "___"),
        (".", ".", "."),
        // single and double character
        ("F", "f", "F"),
        ("PF", "pf", "Pf"),
        ("pF", "pf", "Pf"),
        // underscore prefix
        ("_SomeThing", "_some_thing", "SomeThing"),
        ("_SomeBadMojo", "_some_bad_mojo", "SomeBadMojo"),
        ("_Some_Bad_Mojo", "_some_bad_mojo", "SomeBadMojo"),
        ("_Some_Bad_Mojo__", "_some_bad_mojo__", "SomeBadMojo"),
        // special prefixes (controllable by {CAMEL|SNAKE}WORD_SEPARATOR)
        // note: camel looses next char's case. X3 is this is a problem :)
        ("$SomeThing", "$some_thing", "$someThing"),
        ("@SomeThing", "@some_thing", "@someThing"),
        // string contains word separators specified in {CAMEL|SNAKE}WORD_SEPARATOR)
        ("Some Thing", "some_thing", "SomeThing"),
        ("Some thing", "some_thing", "SomeThing"),
        ("some thing", "some_thing", "SomeThing"),
        ("some Thing", "some_thing", "SomeThing"),
        ("some     thing", "some_thing", "SomeThing"),
        ("some.thing", "some_thing", "SomeThing"),
        ("some:thing", "some_thing", "SomeThing"),
        ("Some::Thing", "some_thing", "SomeThing"),
        ("$Some::Thing", "$some_thing", "$someThing"),
        // should we do something about the below?
        ("$some::Thing", "$some_thing", "$someThing"),
        // Acronym cases
        ("NVMe", "nvme", "Nvme"),
        ("NVME", "nvme", "Nvme"),
        ("nVMEFoobar", "nvme_foobar", "NvmeFoobar"),
        ("iSCSI", "iscsi", "Iscsi"),
        ("iSCSIDriveName", "iscsi_drive_name", "IscsiDriveName"),
        ("PCIe_Functions", "pcie_functions", "PcieFunctions"),
        ("PCIeFunctions", "pcie_functions", "PcieFunctions"),
        ("PCIEFunctions", "pcie_functions", "PcieFunctions"),
        ("PFFunctionNumber", "pf_function_number", "PfFunctionNumber"),
        // Standard cases
        ("FOO_BAR", "foo_bar", "FooBar"),
        ("Foo_Bar", "foo_bar", "FooBar"),
        ("Foo_bar", "foo_bar", "FooBar"),
        ("Foobar", "foobar", "Foobar"),
        ("FooBarBaz", "foo_bar_baz", "FooBarBaz"),
        ("fooBarBaz", "foo_bar_baz", "FooBarBaz"),
        ("PhysFuncNum", "phys_func_num", "PhysFuncNum"),
        ("physFuncNum", "phys_func_num", "PhysFuncNum"),
    ];

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
    fn test_casemungler_common_patterns() {
        for &(input, expected_snake, expected_camel) in TEST_PATTERNS {
            assert_eq!(
                to_snake(input),
                expected_snake,
                "to_snake failed for input: '{}'",
                input
            );
            assert_eq!(
                to_camel(input),
                expected_camel,
                "to_camel failed for input: '{}'",
                input
            );
        }
    }
}
