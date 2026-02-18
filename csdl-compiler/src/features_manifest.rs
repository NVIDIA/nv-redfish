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

//! Features manifest
//!
//! Defines a TOML format that selects which CSDL/EDMX files and
//! entity-type patterns to compile. Intended for build scripts to
//! tailor generated code per product or vendor.

use crate::compiler::EntityTypeFilterPattern;
use serde::Deserialize;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::fs::File;
use std::io::Error as IoError;
use std::io::Read as _;
use std::path::PathBuf;
use toml::de::Error as TomlError;

/// Root manifest describing standard and OEM feature sets.
#[derive(Deserialize, Debug)]
pub struct FeaturesManifest {
    pub features: Vec<Feature>,
    #[serde(rename = "oem-features")]
    pub oem_features: Vec<OemFeature>,
}

impl FeaturesManifest {
    /// Read a features manifest from a TOML file.
    ///
    /// # Errors
    ///
    /// - `Error::Io` if failed to read file
    /// - `Error::Toml` if failed to parse content as TOML / invalid features manifest.
    pub fn read(fname: &PathBuf) -> Result<Self, Error> {
        let mut file = File::open(fname).map_err(Error::Io)?;
        let mut content = String::new();
        file.read_to_string(&mut content).map_err(Error::Io)?;
        toml::from_str(&content).map_err(Error::Toml)
    }

    /// All standard feature names defined in the manifest.
    #[must_use]
    pub fn all_features(&self) -> Vec<&String> {
        self.features.iter().map(|f| &f.name).collect()
    }

    /// Collect standard CSDLs and patterns for selected features.
    #[must_use]
    pub fn collect<'a>(
        &'a self,
        features: &[&String],
    ) -> (
        Vec<&'a String>,
        Vec<&'a String>,
        Vec<&'a EntityTypeFilterPattern>,
        Vec<&'a EntityTypeFilterPattern>,
    ) {
        self.features.iter().fold(
            (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            |(mut files, mut swordfish_files, mut patterns, mut root_patterns), f| {
                if features.contains(&&f.name) {
                    files.extend(f.csdl_files.iter());
                    swordfish_files.extend(f.swordfish_csdl_files.iter());
                    patterns.extend(f.patterns.iter());
                    root_patterns.extend(f.root_patterns.iter());
                }
                (files, swordfish_files, patterns, root_patterns)
            },
        )
    }

    /// All vendors defined by the manifest.
    #[must_use]
    pub fn all_vendors(&self) -> Vec<&String> {
        self.oem_features.iter().map(|f| &f.vendor).collect()
    }

    /// All vendor-specific feature names for a vendor.
    #[must_use]
    pub fn all_vendor_features(&self, vendor: &String) -> Vec<&String> {
        self.oem_features
            .iter()
            .filter_map(|f| {
                if f.vendor == *vendor {
                    Some(&f.name)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect OEM root/resolve CSDLs and patterns for selected features.
    #[must_use]
    pub fn collect_vendor_features<'a>(
        &'a self,
        vendor: &String,
        features: &[&String],
    ) -> (
        Vec<&'a String>, // root csdl
        Vec<&'a String>, // resolve csdl
        Vec<&'a EntityTypeFilterPattern>,
    ) {
        self.oem_features.iter().fold(
            (Vec::new(), Vec::new(), Vec::new()),
            |(mut root, mut resolve, mut patterns), f| {
                if f.vendor == *vendor && features.contains(&&f.name) {
                    root.extend(f.oem_csdl_files.iter());
                    resolve.extend(f.csdl_files.iter());
                    patterns.extend(f.patterns.iter());
                }
                (root, resolve, patterns)
            },
        )
    }
}

/// Standard feature block.
#[derive(Deserialize, Debug)]
pub struct Feature {
    pub name: String,
    pub csdl_files: Vec<String>,
    #[serde(default)]
    pub swordfish_csdl_files: Vec<String>,
    pub patterns: Vec<EntityTypeFilterPattern>,
    #[serde(default)]
    pub root_patterns: Vec<EntityTypeFilterPattern>,
}

/// OEM-specific feature.
#[derive(Deserialize, Debug)]
pub struct OemFeature {
    /// Name of the feature.
    pub name: String,
    /// Vendor name.
    pub vendor: String,
    /// CSDL files provided by vendor that need to be compiled for the
    /// feature.
    pub oem_csdl_files: Vec<String>,
    /// CSDL files from standard that provide types for vendor CSDL
    /// files.
    pub csdl_files: Vec<String>,
    /// Pattern of entity types that need to be resolved during the
    /// compilation.
    #[serde(default)]
    pub patterns: Vec<EntityTypeFilterPattern>,
}

/// Errors reading or parsing the manifest.
#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Toml(TomlError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Io(err) => write!(f, "input/output error: {err}"),
            Self::Toml(err) => write!(f, "manifest file format error: {err}"),
        }
    }
}

impl StdError for Error {}
