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

use crate::compiler::Config as CompilerConfig;
use crate::compiler::EntityTypeFilter;
use crate::compiler::EntityTypeFilterPattern;
use crate::compiler::SchemaBundle;
use crate::edmx::Edmx;
use crate::edmx::ValidateError;
use crate::edmx::attribute_values::Error as AttributeValuesError;
use crate::generator::rust::Config as GeneratorConfig;
use crate::generator::rust::RustGenerator;
use crate::optimizer::optimize;
use clap::Subcommand;
use std::fs::File;
use std::fs::write;
use std::io::Error as IoError;
use std::io::Read as _;
use std::path::PathBuf;

pub const DEFAULT_ROOT: &str = "Service";

#[derive(Debug)]
pub enum Error {
    ParameterNeeded,
    Io(String, IoError),
    Edmx(String, ValidateError),
    Compile(Vec<String>),
    WrongRootService(AttributeValuesError),
    Generate(Vec<String>),
    ParseGenerated(syn::Error),
    WriteOutput(PathBuf, IoError),
}

/// Compiler highlevel commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile CSDL schemas
    Compile {
        /// Root service to be compiled (one of the root singletons in
        /// edm document).
        #[arg(short, long, default_value = DEFAULT_ROOT)]
        root: String,
        /// CSDL documents to be compiled. In most common case you
        /// need to specify all schemas from Redfish and Swordfish
        /// bundles.
        #[arg(required = true)]
        csdls: Vec<String>,
        /// File that contains geneated code.
        #[arg(short, long, default_value = "redfish.rs")]
        output: PathBuf,
        #[arg(short = 'p', long = "pattern")]
        entity_type_patterns: Vec<EntityTypeFilterPattern>,
    },
    /// Compile Oem CSDL schemas
    CompileOem {
        /// CSDL documents to be compiled. All data types from Oem
        /// schema will be compiled.
        #[arg(required = true)]
        csdls: Vec<String>,
        /// File that contains geneated code.
        #[arg(short, long, default_value = "redfish.rs")]
        output: PathBuf,
    },
}

/// Process compiler command.
///
/// # Errors
///
/// If command is failed returns corresponding error.
pub fn process_command(command: &Commands) -> Result<Vec<String>, Error> {
    let mut display_output = Vec::new();
    match command {
        Commands::Compile {
            root,
            csdls,
            output,
            entity_type_patterns,
        } => {
            let root_service = root.parse().map_err(Error::WrongRootService)?;
            if csdls.is_empty() {
                return Err(Error::ParameterNeeded);
            }
            let schema_bundle = read_csdls(csdls)?;
            let compiled = schema_bundle
                .compile(
                    &[root_service],
                    CompilerConfig {
                        entity_type_filter: EntityTypeFilter::new(entity_type_patterns.clone()),
                    },
                )
                .map_err(|e| {
                    format!("{e}")
                        .split('\n')
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .map_err(Error::Compile)?;
            let compiled = optimize(compiled);
            let generator = RustGenerator::new(compiled, GeneratorConfig::default())
                .map_err(|e| {
                    format!("{e}")
                        .split('\n')
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .map_err(Error::Generate)?;

            let result = generator.generate().to_string();
            let syntax_tree = syn::parse_file(&result).map_err(Error::ParseGenerated)?;
            write(output, prettyplease::unparse(&syntax_tree))
                .map_err(|e| Error::WriteOutput(output.clone(), e))?;
            display_output.push(format!("{} file has been written", output.display()));
            Ok(display_output)
        }
        Commands::CompileOem { csdls, output } => {
            if csdls.is_empty() {
                return Err(Error::ParameterNeeded);
            }
            let schema_bundle = read_csdls(csdls)?;
            let compiled = schema_bundle
                .compile_all(CompilerConfig::default())
                .map_err(|e| {
                    format!("{e}")
                        .split('\n')
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .map_err(Error::Compile)?;
            let compiled = optimize(compiled);
            let generator = RustGenerator::new(compiled, GeneratorConfig::default())
                .map_err(|e| {
                    format!("{e}")
                        .split('\n')
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .map_err(Error::Generate)?;

            let result = generator.generate().to_string();
            let syntax_tree = syn::parse_file(&result).map_err(Error::ParseGenerated)?;
            write(output, prettyplease::unparse(&syntax_tree))
                .map_err(|e| Error::WriteOutput(output.clone(), e))?;
            display_output.push(format!("{} file has been written", output.display()));
            Ok(display_output)
        }
    }
}

fn read_csdls(csdls: &[String]) -> Result<SchemaBundle, Error> {
    csdls
        .iter()
        .try_fold(SchemaBundle::default(), |mut schema_bundle, fname| {
            if fname == "@" {
                schema_bundle.root_set_threshold = Some(schema_bundle.edmx_docs.len());
            } else {
                let mut file = File::open(fname).map_err(|err| Error::Io(fname.clone(), err))?;
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .map_err(|err| Error::Io(fname.clone(), err))?;
                schema_bundle
                    .edmx_docs
                    .push(Edmx::parse(&content).map_err(|e| Error::Edmx(fname.clone(), e))?);
            }
            Ok(schema_bundle)
        })
}
