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

//! This crate defines compiler for [Common Schema Definition Language
//! (CSDL)](https://docs.oasis-open.org/odata/odata/v4.0/odata-v4.0-part3-csdl.html)

#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::unimplemented,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]

//#![deny(missing_docs)]

/// Highlevel compiler commands.
pub mod commands;
/// Redfish schema compiler.
pub mod compiler;
/// Entity Data Model XML definitions.
pub mod edmx;
/// Errors of compiler.
pub mod error;
/// Redfish code generator.
pub mod generator;
/// OData-related functions.
pub mod odata;
/// Type or collection of type.
pub mod one_or_collection;
/// Optimizer of compiled data strcutres.
pub mod optimizer;
/// Redfish-related functions.
pub mod redfish;

#[doc(inline)]
pub use error::Error;
#[doc(inline)]
pub use one_or_collection::OneOrCollection;

#[cfg(test)]
mod test {
    use super::edmx::attribute_values::SimpleIdentifier;
    use super::edmx::Edmx;
    use crate::Error;
    use std::fs;
    use std::path::Path;

    fn crate_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn test_edmx_element() -> Result<(), Error> {
        let data = r#"
           <edmx:Edmx Version="4.0">
             <edmx:Reference Uri="http://example.com/1.xml"></edmx:Reference>
             <edmx:Reference Uri="http://example.com/2.xml"></edmx:Reference>
             <edmx:DataServices></edmx:DataServices>
           </edmx:Edmx>"#;
        let edmx: Edmx = Edmx::parse(&data).map_err(|err| Error::Edmx("local".into(), err))?;
        assert!(edmx.data_services.schemas.is_empty());
        Ok(())
    }

    #[test]
    fn test_trivial_data() -> Result<(), Error> {
        let data = r#"
           <edmx:Edmx Version="4.0">
             <edmx:DataServices>
               <Schema Namespace="Org.OData.Core.V1" Alias="Core">
                  <Term Name="Computed" Type="Core.Tag" DefaultValue="true" AppliesTo="Property">
                    <Annotation Term="Core.Description" String="A value for this property is generated on both insert and update"/>
                  </Term>
               </Schema>
             </edmx:DataServices>
           </edmx:Edmx>"#;
        let computed: SimpleIdentifier = "Computed".parse().unwrap();
        let edmx: Edmx = Edmx::parse(&data).map_err(|err| Error::Edmx("local".into(), err))?;
        assert_eq!(edmx.data_services.schemas.len(), 1);
        assert_eq!(edmx.data_services.schemas[0].terms.len(), 1);
        assert!(edmx.data_services.schemas[0].terms.get(&computed).is_some());
        if let Some(term) = &edmx.data_services.schemas[0].terms.get(&computed) {
            assert_eq!(term.ttype.as_ref().unwrap(), &"Core.Tag".parse().unwrap());
            assert_eq!(term.default_value.as_ref().unwrap(), "true");
        }
        Ok(())
    }

    #[ignore]
    #[test]
    fn test_read_odata() -> Result<(), Error> {
        let fname = crate_root().join("test-data/edmx/odata-4.0.xml");
        let fname_string = fname.display().to_string();
        let data = fs::read_to_string(fname).map_err(|err| Error::Io(fname_string.clone(), err))?;
        let _edmx: Edmx = Edmx::parse(&data).map_err(|err| Error::Edmx(fname_string, err))?;
        Ok(())
    }

    #[ignore]
    #[test]
    fn test_read_redfish() -> Result<(), Error> {
        let fname = crate_root().join("test-data/redfish-schema/CoolantConnector_v1.xml");
        let fname_string = fname.display().to_string();
        let data = fs::read_to_string(fname).map_err(|err| Error::Io(fname_string.clone(), err))?;
        let edmx: Edmx = Edmx::parse(&data).map_err(|err| Error::Edmx(fname_string, err))?;
        assert_eq!(edmx.data_services.schemas.len(), 6);
        assert_eq!(edmx.data_services.schemas.get(1).unwrap().types.len(), 4);
        assert_eq!(
            edmx.data_services
                .schemas
                .get(1)
                .unwrap()
                .entity_types
                .len(),
            1
        );
        assert_eq!(
            edmx.data_services.schemas.get(1).unwrap().annotations.len(),
            2
        );
        Ok(())
    }
}
