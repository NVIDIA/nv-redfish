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

//! Update Service entities and collections.
//!
//! This module provides types for working with Redfish UpdateService resources
//! and their sub-resources like firmware and software inventory.

mod software_inventory;

use std::sync::Arc;
use std::time::Duration;

use crate::core::NavProperty;
use crate::patch_support::Payload;
use crate::patch_support::ReadPatchFn;
use crate::schema::update_service::UpdateService as UpdateServiceSchema;
use crate::schema::update_service::UpdateServiceSimpleUpdateAction;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use crate::ServiceRoot;

use nv_redfish_core::Bmc;
use nv_redfish_core::DataStream;
use nv_redfish_core::ModificationResponse;
use nv_redfish_core::MultipartUpdateRequest;
use nv_redfish_core::UploadReader;
use serde_json::Value as JsonValue;
use software_inventory::SoftwareInventoryCollection;

#[doc(inline)]
pub use crate::schema::update_service::TransferProtocolType;
#[doc(inline)]
pub use crate::schema::update_service::UpdateParametersUpdate as MultipartUpdateParameters;
#[doc(inline)]
pub use software_inventory::SoftwareInventory;
#[doc(inline)]
pub use software_inventory::Version;
#[doc(inline)]
pub use software_inventory::VersionRef;

/// Update service.
///
/// Provides functions to access firmware and software inventory, and perform update actions.
pub struct UpdateService<B: Bmc> {
    bmc: NvBmc<B>,
    data: Arc<UpdateServiceSchema>,
    fw_inventory_read_patch_fn: Option<ReadPatchFn>,
}

impl<B: Bmc> UpdateService<B> {
    /// Create a new update service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        root: &ServiceRoot<B>,
    ) -> Result<Option<Self>, Error<B>> {
        let mut service_patches = Vec::new();
        if bmc.quirks.bug_missing_update_service_name_field() {
            service_patches.push(add_default_update_service_name);
        }
        let service_patch_fn = (!service_patches.is_empty()).then(|| {
            Arc::new(move |v| service_patches.iter().fold(v, |acc, f| f(acc))) as ReadPatchFn
        });

        let mut fw_inventory_patches = Vec::new();
        if bmc.quirks.fw_inventory_wrong_release_date() {
            fw_inventory_patches.push(fw_inventory_patch_wrong_release_date);
        }
        let fw_inventory_read_patch_fn = (!fw_inventory_patches.is_empty()).then(|| {
            Arc::new(move |v| fw_inventory_patches.iter().fold(v, |acc, f| f(acc))) as ReadPatchFn
        });

        if let Some(nav) = &root.root.update_service {
            if let Some(service_patch_fn) = service_patch_fn {
                Payload::get(bmc.as_ref(), nav, service_patch_fn.as_ref()).await
            } else {
                nav.get(bmc.as_ref()).await.map_err(Error::Bmc)
            }
            .map(Some)
        } else if bmc.quirks.bug_missing_root_nav_properties() {
            let nav =
                NavProperty::new_reference(format!("{}/UpdateService", root.odata_id()).into());
            if let Some(service_patch_fn) = service_patch_fn {
                Payload::get(bmc.as_ref(), &nav, service_patch_fn.as_ref()).await
            } else {
                nav.get(bmc.as_ref()).await.map_err(Error::Bmc)
            }
            .map(Some)
        } else {
            Ok(None)
        }
        .map(|d| {
            d.map(|data| Self {
                bmc: bmc.clone(),
                data,
                fw_inventory_read_patch_fn,
            })
        })
    }

    /// Get the raw schema data for this update service.
    ///
    /// Returns an `Arc` to the underlying schema, allowing cheap cloning
    /// and sharing of the data.
    #[must_use]
    pub fn raw(&self) -> Arc<UpdateServiceSchema> {
        self.data.clone()
    }

    /// List all firmware inventory items.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not have a firmware inventory collection
    /// - Fetching firmware inventory data fails
    pub async fn firmware_inventories(
        &self,
    ) -> Result<Option<Vec<SoftwareInventory<B>>>, Error<B>> {
        if let Some(collection_ref) = &self.data.firmware_inventory {
            SoftwareInventoryCollection::new(
                &self.bmc,
                collection_ref,
                self.fw_inventory_read_patch_fn.clone(),
            )
            .await?
            .members()
            .await
            .map(Some)
        } else {
            Ok(None)
        }
    }

    /// List all software inventory items.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not have a software inventory collection
    /// - Fetching software inventory data fails
    pub async fn software_inventories(
        &self,
    ) -> Result<Option<Vec<SoftwareInventory<B>>>, Error<B>> {
        if let Some(collection_ref) = &self.data.software_inventory {
            let collection = self.bmc.expand_property(collection_ref).await?;
            let mut items = Vec::new();
            for item_ref in &collection.members {
                items.push(SoftwareInventory::new(&self.bmc, item_ref, None).await?);
            }
            Ok(Some(items))
        } else {
            Ok(None)
        }
    }

    /// Perform a simple update with the specified image URI.
    ///
    /// This action updates software components by downloading and installing
    /// a software image from the specified URI.
    ///
    /// # Arguments
    ///
    /// * `image_uri` - The URI of the software image to install
    /// * `transfer_protocol` - Optional network protocol to use for retrieving the image
    /// * `targets` - Optional list of URIs indicating where to apply the update
    /// * `username` - Optional username for accessing the image URI
    /// * `password` - Optional password for accessing the image URI
    /// * `force_update` - Whether to bypass update policies (e.g., allow downgrade)
    /// * `stage` - Whether to stage the image for later activation instead of immediate
    ///   installation
    /// * `local_image` - An indication of whether the service adds the image to the local image
    ///   store
    /// * `exclude_targets` - An array of URIs that indicate where not to apply the update image
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not support the `SimpleUpdate` action
    /// - The action execution fails
    #[allow(clippy::too_many_arguments)]
    pub async fn simple_update(
        &self,
        image_uri: String,
        transfer_protocol: Option<TransferProtocolType>,
        targets: Option<Vec<String>>,
        username: Option<String>,
        password: Option<String>,
        force_update: Option<bool>,
        stage: Option<bool>,
        local_image: Option<bool>,
        exclude_targets: Option<Vec<String>>,
    ) -> Result<ModificationResponse<()>, Error<B>>
    where
        B::Error: nv_redfish_core::ActionError,
    {
        let actions = self
            .data
            .actions
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;

        actions
            .simple_update(
                self.bmc.as_ref(),
                &UpdateServiceSimpleUpdateAction {
                    image_uri: Some(image_uri),
                    transfer_protocol,
                    targets,
                    username,
                    password,
                    force_update,
                    stage,
                    local_image,
                    exclude_targets,
                },
            )
            .await
            .map_err(Error::Bmc)
    }

    /// Start updates that have been previously invoked with an `OperationApplyTime` of
    /// `OnStartUpdateRequest`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update service does not support the `StartUpdate` action
    /// - The action execution fails
    pub async fn start_update(&self) -> Result<ModificationResponse<()>, Error<B>>
    where
        B::Error: nv_redfish_core::ActionError,
    {
        let actions = self
            .data
            .actions
            .as_ref()
            .ok_or(Error::ActionNotAvailable)?;

        actions
            .start_update(self.bmc.as_ref())
            .await
            .map_err(Error::Bmc)
    }

    /// Upload a named stream using this service's `MultipartHttpPushUri`.
    ///
    /// Prefer the generated [`MultipartUpdateParameters`] type. A generic
    /// payload is accepted for platform fields that are not generated.
    ///
    /// # Errors
    ///
    /// Returns an error if `MultipartHttpPushUri` is absent or the upload fails.
    pub async fn multipart_update_from_reader<U, V, R>(
        &self,
        update_parameters: &V,
        update_stream: DataStream<U>,
        upload_timeout: Duration,
    ) -> Result<ModificationResponse<R>, Error<B>>
    where
        U: UploadReader,
        V: Send + Sync + serde::Serialize,
        R: Send + Sync + for<'de> serde::Deserialize<'de>,
    {
        self.multipart_update(MultipartUpdateRequest {
            update_parameters,
            update_stream,
            oem_parts: Vec::new(),
            upload_timeout,
        })
        .await
    }

    /// Perform a multipart upload using this service's `MultipartHttpPushUri`.
    ///
    /// Use this method when the request needs optional OEM multipart parts.
    ///
    /// # Errors
    ///
    /// Returns an error if `MultipartHttpPushUri` is absent or the upload fails.
    pub async fn multipart_update<U, V, R>(
        &self,
        request: MultipartUpdateRequest<'_, U, V>,
    ) -> Result<ModificationResponse<R>, Error<B>>
    where
        U: UploadReader,
        V: Send + Sync + serde::Serialize,
        R: Send + Sync + for<'de> serde::Deserialize<'de>,
    {
        let multipart_uri = self
            .data
            .multipart_http_push_uri
            .as_ref()
            .ok_or(Error::UpdateServiceMultipartHttpPushUriNotAvailable)?;

        self.bmc
            .as_ref()
            .multipart_update(multipart_uri, request)
            .await
            .map_err(Error::Bmc)
    }
}

impl<B: Bmc> Resource for UpdateService<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}

// `ReleaseDate` is marked as `edm.DateTimeOffset`, but some systems
// puts "00:00:00Z" as ReleaseDate that is not conform to ABNF of the DateTimeOffset.
// we delete such fields...
fn fw_inventory_patch_wrong_release_date(v: JsonValue) -> JsonValue {
    if let JsonValue::Object(mut obj) = v {
        if let Some(JsonValue::String(date)) = obj.get("ReleaseDate") {
            if date == "00:00:00Z" || date == "0000-00-00T00:00:00Z" {
                obj.remove("ReleaseDate");
            }
        }
        JsonValue::Object(obj)
    } else {
        v
    }
}

fn add_default_update_service_name(v: JsonValue) -> JsonValue {
    if let JsonValue::Object(mut obj) = v {
        obj.entry("Name")
            .or_insert(JsonValue::String("Unnamed update service".into()));
        JsonValue::Object(obj)
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::io::Cursor;
    use nv_redfish_bmc_mock::Bmc as MockBmc;
    use nv_redfish_bmc_mock::Expect;
    use nv_redfish_core::OemMultipartPart;
    use serde_json::json;
    use std::error::Error as StdError;
    use std::io;

    const UPDATE_SERVICE_URI: &str = "/redfish/v1/UpdateService";
    const MULTIPART_URI: &str = "/redfish/v1/UpdateService/update-multipart";

    type TestResult = Result<(), Box<dyn StdError>>;

    #[tokio::test]
    async fn uses_multipart_http_push_uri() -> TestResult {
        let bmc = Arc::new(MockBmc::<io::Error>::default());
        bmc.expect(Expect::get("/redfish/v1", service_root_json()));
        bmc.expect(Expect::get(
            UPDATE_SERVICE_URI,
            update_service_json(Some(MULTIPART_URI)),
        ));
        bmc.expect(Expect::multipart_update(
            MULTIPART_URI,
            json!({
                "ForceUpdate": true,
                "Targets": ["/redfish/v1/Systems/1"]
            }),
            "firmware.bin",
            json!({
                "@odata.id": "/redfish/v1/TaskService/Tasks/42",
                "Id": "42"
            }),
        ));

        let root = ServiceRoot::new(Arc::clone(&bmc)).await?;
        let update_service = root
            .update_service()
            .await?
            .ok_or("expected update service")?;
        let parameters = MultipartUpdateParameters::builder()
            .with_force_update(true)
            .with_targets(vec!["/redfish/v1/Systems/1".to_string()])
            .build();

        let response = update_service
            .multipart_update_from_reader::<_, _, serde_json::Value>(
                &parameters,
                DataStream::new("firmware.bin", Cursor::new(b"firmware".to_vec()))
                    .with_content_length(8),
                Duration::from_secs(600),
            )
            .await?;

        let ModificationResponse::Entity(body) = response else {
            return Err(String::from("expected entity response").into());
        };

        assert_eq!(body["@odata.id"], "/redfish/v1/TaskService/Tasks/42");

        Ok(())
    }

    #[tokio::test]
    async fn uses_generated_update_parameters_with_oem_parts() -> TestResult {
        let bmc = Arc::new(MockBmc::<io::Error>::default());
        bmc.expect(Expect::get("/redfish/v1", service_root_json()));
        bmc.expect(Expect::get(
            UPDATE_SERVICE_URI,
            update_service_json(Some(MULTIPART_URI)),
        ));

        let oem = crate::schema::resource::OemUpdate {
            additional_properties: json!({
                "Nvidia": {
                    "Mode": "Rms"
                }
            }),
        };
        let parameters = MultipartUpdateParameters::builder()
            .with_force_update(true)
            .with_targets(vec!["/redfish/v1/Systems/1".to_string()])
            .with_oem(oem)
            .build();

        let expected_parameters = serde_json::to_value(&parameters)?;

        bmc.expect(Expect::multipart_update_with_oem_parts(
            MULTIPART_URI,
            expected_parameters,
            "firmware.bin",
            ["OemNvidia"],
            json!({
                "@odata.id": "/redfish/v1/TaskService/Tasks/77",
                "Id": "77"
            }),
        ));

        let root = ServiceRoot::new(Arc::clone(&bmc)).await?;
        let update_service = root
            .update_service()
            .await?
            .ok_or("expected update service")?;

        let request = MultipartUpdateRequest {
            update_parameters: &parameters,
            update_stream: DataStream::new("firmware.bin", Cursor::new(b"firmware".to_vec())),
            oem_parts: vec![OemMultipartPart::new(
                "OemNvidia",
                Cursor::new(br#"{"Mode":"Rms"}"#.to_vec()),
            )?
            .with_content_type("application/json")],
            upload_timeout: Duration::from_secs(600),
        };

        let response = update_service
            .multipart_update::<_, _, serde_json::Value>(request)
            .await?;

        let ModificationResponse::Entity(body) = response else {
            return Err(String::from("expected entity response").into());
        };

        assert_eq!(body["@odata.id"], "/redfish/v1/TaskService/Tasks/77");

        Ok(())
    }

    #[tokio::test]
    async fn requires_multipart_http_push_uri() -> TestResult {
        let bmc = Arc::new(MockBmc::<io::Error>::default());

        bmc.expect(Expect::get("/redfish/v1", service_root_json()));

        // Multipart uploads must use the service-provided endpoint, so a
        // missing MultipartHttpPushUri should fail before any upload is sent.
        bmc.expect(Expect::get(UPDATE_SERVICE_URI, update_service_json(None)));

        let root = ServiceRoot::new(Arc::clone(&bmc)).await?;
        let update_service = root
            .update_service()
            .await?
            .ok_or("expected update service")?;

        let result = update_service
            .multipart_update_from_reader::<_, _, serde_json::Value>(
                &MultipartUpdateParameters::default(),
                DataStream::new("firmware.bin", Cursor::new(b"firmware".to_vec()))
                    .with_content_length(8),
                Duration::from_secs(600),
            )
            .await;

        assert!(matches!(
            result,
            Err(Error::UpdateServiceMultipartHttpPushUriNotAvailable)
        ));

        Ok(())
    }

    fn service_root_json() -> serde_json::Value {
        json!({
            "@odata.id": "/redfish/v1",
            "Id": "RootService",
            "Name": "Root Service",
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                }
            },
            "UpdateService": {
                "@odata.id": UPDATE_SERVICE_URI
            }
        })
    }

    fn update_service_json(multipart_uri: Option<&str>) -> serde_json::Value {
        let mut body = json!({
            "@odata.id": UPDATE_SERVICE_URI,
            "Id": "UpdateService",
            "Name": "Update Service"
        });

        if let Some(multipart_uri) = multipart_uri {
            body["MultipartHttpPushUri"] = json!(multipart_uri);
        }

        body
    }
}
