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

use std::marker::PhantomData;

use tagged_types::TaggedType;

/// Type for hardware manufacturers.
pub type Manufacturer<T, Tag> = TaggedType<T, ManufacturerTag<Tag>>;
#[doc(hidden)]
pub struct ManufacturerTag<Tag> {
    _marker: PhantomData<Tag>,
}
impl<T> tagged_types::ImplementClone for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementCopy for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementHash for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementPartialEq for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementEq for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementPartialOrd for ManufacturerTag<T> {}
impl<T> tagged_types::ImplementDeref for ManufacturerTag<T> {}
impl<T> tagged_types::TransparentDebug for ManufacturerTag<T> {}
impl<T> tagged_types::TransparentDisplay for ManufacturerTag<T> {}
impl<T> tagged_types::TransparentSerialize for ManufacturerTag<T> {}
impl<T> tagged_types::TransparentDeserialize for ManufacturerTag<T> {}
impl<T> tagged_types::InnerAccess for ManufacturerTag<T> {}
impl<T> tagged_types::Cloned for ManufacturerTag<T> {}
impl<T> tagged_types::AsRef for ManufacturerTag<T> {}
impl<T> tagged_types::ValueMap for ManufacturerTag<T> {}

/// Type for hardware model.
pub type Model<T, Tag> = TaggedType<T, ModelTag<Tag>>;
#[doc(hidden)]
pub struct ModelTag<Tag> {
    _marker: PhantomData<Tag>,
}
impl<T> tagged_types::ImplementClone for ModelTag<T> {}
impl<T> tagged_types::ImplementCopy for ModelTag<T> {}
impl<T> tagged_types::ImplementHash for ModelTag<T> {}
impl<T> tagged_types::ImplementPartialEq for ModelTag<T> {}
impl<T> tagged_types::ImplementEq for ModelTag<T> {}
impl<T> tagged_types::ImplementPartialOrd for ModelTag<T> {}
impl<T> tagged_types::TransparentDebug for ModelTag<T> {}
impl<T> tagged_types::TransparentDisplay for ModelTag<T> {}
impl<T> tagged_types::TransparentSerialize for ModelTag<T> {}
impl<T> tagged_types::TransparentDeserialize for ModelTag<T> {}
impl<T> tagged_types::InnerAccess for ModelTag<T> {}
impl<T> tagged_types::Cloned for ModelTag<T> {}
impl<T> tagged_types::AsRef for ModelTag<T> {}
impl<T> tagged_types::ValueMap for ModelTag<T> {}

/// Type for hardware model.
pub type PartNumber<T, Tag> = TaggedType<T, PartNumberTag<Tag>>;
#[doc(hidden)]
pub struct PartNumberTag<Tag> {
    _marker: PhantomData<Tag>,
}
impl<T> tagged_types::ImplementClone for PartNumberTag<T> {}
impl<T> tagged_types::ImplementCopy for PartNumberTag<T> {}
impl<T> tagged_types::ImplementHash for PartNumberTag<T> {}
impl<T> tagged_types::ImplementPartialEq for PartNumberTag<T> {}
impl<T> tagged_types::ImplementEq for PartNumberTag<T> {}
impl<T> tagged_types::ImplementPartialOrd for PartNumberTag<T> {}
impl<T> tagged_types::TransparentDebug for PartNumberTag<T> {}
impl<T> tagged_types::TransparentDisplay for PartNumberTag<T> {}
impl<T> tagged_types::TransparentSerialize for PartNumberTag<T> {}
impl<T> tagged_types::TransparentDeserialize for PartNumberTag<T> {}
impl<T> tagged_types::InnerAccess for PartNumberTag<T> {}
impl<T> tagged_types::Cloned for PartNumberTag<T> {}
impl<T> tagged_types::AsRef for PartNumberTag<T> {}
impl<T> tagged_types::ValueMap for PartNumberTag<T> {}

/// Type for hardware serial numbers.
pub type SerialNumber<T, Tag> = TaggedType<T, SerialNumberTag<Tag>>;
#[doc(hidden)]
pub struct SerialNumberTag<Tag> {
    _marker: PhantomData<Tag>,
}
impl<T> tagged_types::ImplementClone for SerialNumberTag<T> {}
impl<T> tagged_types::ImplementCopy for SerialNumberTag<T> {}
impl<T> tagged_types::ImplementHash for SerialNumberTag<T> {}
impl<T> tagged_types::ImplementPartialEq for SerialNumberTag<T> {}
impl<T> tagged_types::ImplementEq for SerialNumberTag<T> {}
impl<T> tagged_types::ImplementPartialOrd for SerialNumberTag<T> {}
impl<T> tagged_types::TransparentDebug for SerialNumberTag<T> {}
impl<T> tagged_types::TransparentDisplay for SerialNumberTag<T> {}
impl<T> tagged_types::TransparentSerialize for SerialNumberTag<T> {}
impl<T> tagged_types::TransparentDeserialize for SerialNumberTag<T> {}
impl<T> tagged_types::InnerAccess for SerialNumberTag<T> {}
impl<T> tagged_types::Cloned for SerialNumberTag<T> {}
impl<T> tagged_types::AsRef for SerialNumberTag<T> {}
impl<T> tagged_types::ValueMap for SerialNumberTag<T> {}

/// Hardware ID is Manufacturer + Model + Part Number + Serial Number.
/// It is tagged by the type of related redfish module.
#[derive(Clone)]
pub struct HardwareId<Tag> {
    /// Manufacturer of the hardware.
    pub manufacturer: Option<Manufacturer<String, Tag>>,
    /// Model of the hardware.
    pub model: Option<Model<String, Tag>>,
    /// Part number assigned by the manufacturer
    pub part_number: Option<PartNumber<String, Tag>>,
    /// Serial number assigned by the manufacturer
    pub serial_number: Option<SerialNumber<String, Tag>>,
}

/// Reference to hardware IDs.
#[derive(Clone, Copy)]
pub struct HardwareIdRef<'a, Tag> {
    /// Manufacturer of the hardware.
    pub manufacturer: Option<Manufacturer<&'a str, Tag>>,
    /// Model of the hardware.
    pub model: Option<Model<&'a str, Tag>>,
    /// Part number assigned by the manufacturer
    pub part_number: Option<PartNumber<&'a str, Tag>>,
    /// Serial number assigned by the manufacturer
    pub serial_number: Option<SerialNumber<&'a str, Tag>>,
}

impl<Tag> HardwareIdRef<'_, Tag> {
    /// Transform to owned `HardwareId`.
    #[must_use]
    pub fn cloned(&self) -> HardwareId<Tag> {
        HardwareId {
            manufacturer: self.manufacturer.map(|v| v.map(ToString::to_string)),
            model: self.model.map(|v| v.map(ToString::to_string)),
            part_number: self.part_number.map(|v| v.map(ToString::to_string)),
            serial_number: self.serial_number.map(|v| v.map(ToString::to_string)),
        }
    }
}
