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

use crate::Error;
use crate::compiler::Config as CompilerConfig;
use crate::compiler::EntityTypeFilter;
use crate::compiler::EntityTypeFilterPattern;
use crate::compiler::SchemaBundle;
use crate::edmx::Edmx;
use crate::generator::rust::Config as GeneratorConfig;
use crate::generator::rust::RustGenerator;
use crate::optimizer::optimize;
use clap::Subcommand;
use std::fs::File;
use std::fs::write;
use std::io::Read as _;
use std::path::PathBuf;

pub const DEFAULT_ROOT: &str = "Service";

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
        /// Defines patterns of entity types that should be compiled
        /// if referenced via navigation property. If this list is
        /// empty then all entity types will be compiled.
        ///
        /// Pattern is wildcard of qualified name.
        /// Examples:
        /// `ServiceRoot.*.*` - any entity type in any version of the service root
        /// `SomeNamespace.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of namespace `SomeNamespace`.
        /// `*.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of any namespaces.
        #[arg(short = 'p', long = "pattern")]
        entity_type_patterns: Vec<EntityTypeFilterPattern>,
    },
    /// Compile Oem CSDL schemas
    CompileOem {
        /// CSDL documents to be compiled and included to root
        /// set (all data types from Oem schema will be compiled).
        #[arg(required = true, value_terminator = "@")]
        root_csdls: Vec<String>,
        /// CSDL documents to for type resolution in `root_csdls`.
        #[arg(index = 2)]
        resolve_csdls: Vec<String>,
        /// File that contains geneated code.
        #[arg(short, long, default_value = "redfish.rs")]
        output: PathBuf,
        /// Defines patterns of entity types that should be compiled
        /// if referenced via navigation property. If this list is
        /// empty then all entity types will be compiled.
        ///
        /// Pattern is wildcard of qualified name.
        /// Examples:
        /// `ServiceRoot.*.*` - any entity type in any version of the service root
        /// `SomeNamespace.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of namespace `SomeNamespace`.
        /// `*.*.Entity1|Entity2` - `EntityType1` or `EntityType2` from any versions of any namespaces.
        #[arg(short = 'p', long = "pattern")]
        entity_type_patterns: Vec<EntityTypeFilterPattern>,
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
                return Err(Error::AtLeastOneCSDLFileNeeded);
            }
            let schema_bundle = read_csdls(&[], csdls)?;
            let compiled = schema_bundle
                .compile(
                    &[root_service],
                    CompilerConfig {
                        entity_type_filter: EntityTypeFilter::new(entity_type_patterns.clone()),
                    },
                )
                .map_err(Error::compile_error)?;
            let compiled = optimize(compiled);
            let generator = RustGenerator::new(compiled, GeneratorConfig::default())
                .map_err(Error::generate_error)?;

            let result = generator.generate().to_string();
            let syntax_tree = syn::parse_file(&result).map_err(Error::ParseGenerated)?;
            write(output, prettyplease::unparse(&syntax_tree))
                .map_err(|e| Error::WriteOutput(output.clone(), e))?;
            display_output.push(format!("{} file has been written", output.display()));
            Ok(display_output)
        }
        Commands::CompileOem {
            root_csdls,
            resolve_csdls,
            output,
            entity_type_patterns,
        } => {
            if root_csdls.is_empty() {
                return Err(Error::AtLeastOneCSDLFileNeeded);
            }
            let schema_bundle = read_csdls(root_csdls, resolve_csdls)?;
            let compiled = schema_bundle
                .compile_all(CompilerConfig {
                    entity_type_filter: EntityTypeFilter::new(entity_type_patterns.clone()),
                })
                .map_err(Error::compile_error)?;
            let compiled = optimize(compiled);
            let generator = RustGenerator::new(compiled, GeneratorConfig::default())
                .map_err(Error::generate_error)?;
            let result = generator.generate().to_string();
            let syntax_tree = syn::parse_file(&result).map_err(Error::ParseGenerated)?;
            write(output, prettyplease::unparse(&syntax_tree))
                .map_err(|e| Error::WriteOutput(output.clone(), e))?;
            display_output.push(format!("{} file has been written", output.display()));
            Ok(display_output)
        }
    }
}

fn read_csdls(root_csdls: &[String], resolve_csdls: &[String]) -> Result<SchemaBundle, Error> {
    let edmx_docs = root_csdls
        .iter()
        .chain(resolve_csdls.iter())
        .map(|fname| {
            let mut file = File::open(fname).map_err(|err| Error::Io(fname.clone(), err))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|err| Error::Io(fname.clone(), err))?;
            Edmx::parse(&content).map_err(|e| Error::Edmx(fname.clone(), e))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SchemaBundle {
        edmx_docs,
        root_set_threshold: if root_csdls.is_empty() {
            None
        } else {
            Some(root_csdls.len())
        },
    })
}
