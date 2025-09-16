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

use csdl_compiler::compiler::CompiledPropertyType;
use csdl_compiler::compiler::SchemaBundle;
use csdl_compiler::compiler::SimpleTypeAttrs;
use csdl_compiler::edmx::Edmx;
use csdl_compiler::edmx::ValidateError;
use csdl_compiler::edmx::attribute_values::Error as AttributeValuesError;
use csdl_compiler::optimizer::optimize;
use std::io::Error as IoError;
use std::io::Read;

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    ParameterNeeded,
    Io(String, IoError),
    Edmx(String, ValidateError),
    Compile(String),
    WrongRootService(AttributeValuesError),
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Usage:");
        println!(" {} <root service> <redfish-csdl-file> ...", args[0]);
        return Err(Error::ParameterNeeded);
    }
    let root_service = args[1].parse().map_err(Error::WrongRootService)?;
    let schema_bundle =
        args[2..]
            .iter()
            .try_fold(SchemaBundle::default(), |mut schema_bundle, fname| {
                let mut file =
                    std::fs::File::open(fname).map_err(|err| Error::Io(fname.clone(), err))?;
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .map_err(|err| Error::Io(fname.clone(), err))?;
                schema_bundle
                    .edmx_docs
                    .push(Edmx::parse(&content).map_err(|e| Error::Edmx(fname.clone(), e))?);
                Ok(schema_bundle)
            })?;
    let compiled = schema_bundle
        .compile(&[root_service])
        .inspect_err(|e| println!("{e}"))
        .map_err(|_| Error::Compile("compilation error".into()))?;
    let compiled = optimize(compiled);

    println!("Simple types:");
    for t in compiled.simple_types.values() {
        print!("  {}: ", t.name);
        match &t.attrs {
            SimpleTypeAttrs::EnumType(v) => println!("Enum ({:?})", v.underlying_type),
            SimpleTypeAttrs::TypeDefinition(v) => println!("Typedef ({})", v.underlying_type),
        }
    }
    println!();
    println!("Complex types:");
    for t in compiled.complex_types.values() {
        print!("  {}", t.name);
        if let Some(base) = t.base {
            print!(" extends {base}");
        }
        println!();
        if !t.properties.is_empty() {
            println!("    properties:");
            for p in &t.properties {
                match p.ptype {
                    CompiledPropertyType::One(t) => println!("      {}: {}", p.name, t),
                    CompiledPropertyType::CollectionOf(t) => println!("      {}: {}[]", p.name, t),
                }
            }
        }
        if !t.nav_properties.is_empty() {
            println!("    Nav properties:");
            for p in &t.nav_properties {
                match p.ptype {
                    CompiledPropertyType::One(t) => println!("      {}: {}", p.name, t),
                    CompiledPropertyType::CollectionOf(t) => println!("      {}: {}[]", p.name, t),
                }
            }
        }
    }
    println!();
    println!("Entity types:");
    for t in compiled.entity_types.values() {
        print!("  {}", t.name);
        if let Some(base) = t.base {
            print!(" extends {base}");
        }
        println!();
        if !t.properties.is_empty() {
            println!("    properties:");
            for p in &t.properties {
                match p.ptype {
                    CompiledPropertyType::One(t) => println!("      {}: {}", p.name, t),
                    CompiledPropertyType::CollectionOf(t) => println!("      {}: {}[]", p.name, t),
                }
            }
        }
        if !t.nav_properties.is_empty() {
            println!("    Nav properties:");
            for p in &t.nav_properties {
                match p.ptype {
                    CompiledPropertyType::One(t) => println!("      {}: {}", p.name, t),
                    CompiledPropertyType::CollectionOf(t) => println!("      {}: {}[]", p.name, t),
                }
            }
        }
    }
    println!();
    println!("Singletons:");
    for s in &compiled.root_singletons {
        println!("  {} of {}", s.name, s.stype);
    }
    println!();
    println!("Statistics:");
    println!(" complex types:   {}", compiled.complex_types.len());
    println!(" entity types:    {}", compiled.entity_types.len());
    println!(" simple types:    {}", compiled.simple_types.len());
    println!(" root singletons: {}", compiled.root_singletons.len());
    Ok(())
}
