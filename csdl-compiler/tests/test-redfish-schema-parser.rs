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

use csdl_compiler::edmx::Edmx;
use csdl_compiler::edmx::ValidateError;
use csdl_compiler::generator::RedfishResource;
use std::io::Error as IoError;
use std::io::Read;

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    ParameterNeeded,
    Io(String, IoError),
    Edmx(ValidateError),
    RedfishResource(String),
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage:");
        println!(" {} <redfish-csdl-file>", args[0]);
        return Err(Error::ParameterNeeded);
    }
    let fname = args[1].clone();
    let mut file =
        std::fs::File::open(args[1].clone()).map_err(|err| Error::Io(fname.clone(), err))?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|err| Error::Io(fname.clone(), err))?;
    let edmx = Edmx::parse(&content).map_err(Error::Edmx)?;
    let (resources, type_registry) =
        RedfishResource::from_edmx(&edmx).map_err(Error::RedfishResource)?;
    println!("========================= Type Registry ===========================================");
    println!("{type_registry:#?}");
    println!("=========================== Resources =============================================");
    println!("{resources:#?}");
    Ok(())
}
