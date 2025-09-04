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

//

pub mod edmx;

use quick_xml::DeError;
use std::io::Error as IoError;

#[derive(Debug)]
pub enum Error {
    XmlDeserialize(DeError),
    FileRead(IoError),
}

#[cfg(test)]
mod test {
    use super::Error;
    use super::edmx::Edmx;
    use super::edmx::SchemaItem;
    use std::fs;
    use std::path::Path;

    fn crate_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
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
        let edmx: Edmx = quick_xml::de::from_str(&data).map_err(Error::XmlDeserialize)?;
        assert_eq!(edmx.data_services.schemas.len(), 1);
        assert_eq!(edmx.data_services.schemas[0].items.len(), 1);
        assert!(matches!(
            edmx.data_services.schemas[0].items[0],
            SchemaItem::Term(..)
        ));
        if let SchemaItem::Term(term) = &edmx.data_services.schemas[0].items[0] {
            assert_eq!(term.name, "Computed");
            assert_eq!(term.ttype.as_ref().unwrap(), "Core.Tag");
            assert_eq!(term.default_value.as_ref().unwrap(), "true");
            assert_eq!(term.applies_to.as_ref().unwrap(), "Property");
        }
        Ok(())
    }

    #[ignore]
    #[test]
    fn test_read_odata() -> Result<(), Error> {
        let xml = fs::read_to_string(crate_root().join("test-data/edmx/odata-4.0.xml"))
            .map_err(Error::FileRead)?;
        let _edmx: Edmx = quick_xml::de::from_str(&xml).map_err(Error::XmlDeserialize)?;
        Ok(())
    }
}
