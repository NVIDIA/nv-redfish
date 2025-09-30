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

use std::sync::Arc;

use nv_redfish::Bmc;
use nv_redfish::Expandable;
use nv_redfish::ODataId;
use nv_redfish::http::ExpandQuery;
use redfish_std::redfish::service_root::ServiceRoot;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug)]
pub enum Error {
    GenericError,
    NotFound,
    NetworkError,
    AuthError,
    ParseError(serde_json::Error),
}

#[derive(Debug, Default)]
pub struct MockBmc {}

impl MockBmc {
    pub async fn get_service_root(&self) -> Result<Arc<ServiceRoot>, Error> {
        nv_redfish::NavProperty::<redfish_std::redfish::service_root::ServiceRoot>::new_reference(
            ODataId::service_root(),
        )
        .get(self)
        .await
    }

    fn get_mock_json_for_uri(&self, uri: &str) -> String {
        match uri {
            "/redfish/v1" => {
                r##"{
                      "@odata.id": "/redfish/v1",
                      "Id": "RootService",
                      "Name": "Root Service",
                      "RedfishVersion": "1.19.0",
                      "UUID": "12345678-1234-1234-1234-123456789012",
                      "Chassis": {"@odata.id": "/redfish/v1/Chassis"},
                      "Systems": {"@odata.id": "/redfish/v1/Systems"},
                      "Links": {
                         "Sessions": {"@odata.id": "/redfish/v1/SessionService/Sessions"}
                      },
                      "Oem": {
                          "Contoso": {
                              "@odata.type": "#ContosoServiceRoot.v1_0_0.ServiceRoot",
                              "TurboencabulatorService": {
                                  "@odata.id": "/redfish/v1/Oem/Contoso/TurboencabulatorService"
                              }
                          }
                      }
                   }"##.to_string()
            },
            "/redfish/v1/Chassis" => {
                r#"{
                      "@odata.id": "/redfish/v1/Chassis",
                      "Name": "Chassis Collection",
                      "Members": [
                          {
                              "@odata.id": "/redfish/v1/Chassis/1"
                          },
                          {
                              "@odata.id": "/redfish/v1/Chassis/2"
                          }
                      ]
                }"#.to_string()
            },
            "/redfish/v1/Chassis/1" => {
                let chassis_id = "1";
                format!(r#"{{
                   "@odata.id": "/redfish/v1/Chassis/{chassis_id}",
                   "Id": "{chassis_id}",
                   "Name": "Chassis {chassis_id}",
                   "ChassisType": "Rack",
                   "Manufacturer": "NVIDIA",
                   "Model": "DGX-H100",
                   "SerialNumber": "ABC123-{chassis_id}",
                   "PCIeDevices": {{
                       "@odata.id": "/redfish/v1/Chassis/{chassis_id}/PCIeDevices"
                   }},
                   "Status": {{"State": "Enabled", "Health": "OK"}}
                }}"#)
            },
            "/redfish/v1/Chassis/1/PCIeDevices" => {
                r#"{
                      "@odata.id": "/redfish/v1/Chassis/1/PCIeDevices",
                      "Name": "Chassis Collection",
                      "Members": [
                          {
                              "@odata.id": "/redfish/v1/Chassis/1/PCIeDevices/0-24"
                          },
                          {
                              "@odata.id": "/redfish/v1/Chassis/1/PCIeDevices/0-25"
                          }
                      ]
                }"#.to_string()
            },
            "/redfish/v1/Chassis/1/PCIeDevices/0-24" => {
                r#"{
                     "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24",
                     "Id": "0-24",
                     "Links": {
                     },
                     "Manufacturer": "Intel Corporation",
                     "Model": null,
                     "Name": "Sapphire Rapids SATA AHCI Controller",
                     "PartNumber": null,
                     "SKU": null,
                     "SerialNumber": null,
                     "Slot": {},
                     "Status": {
                         "State": "Enabled",
                         "Health": "OK",
                         "HealthRollup": "OK"
                     },
                     "PCIeFunctions": {
                         "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions"
                     }
                }"#.to_string()
            },
            "/redfish/v1/Chassis/1/PCIeDevices/0-25" => {
                r##"{
                    "@odata.context": "/redfish/v1/$metadata#PCIeDevice.PCIeDevice",
                    "@odata.etag": "\"1754525527\"",
                    "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-25",
                    "@odata.type": "#PCIeDevice.v1_14_0.PCIeDevice",
                    "AssetTag": null,
                    "Description": "Sapphire Rapids SATA AHCI Controller",
                    "DeviceType": "SingleFunction",
                    "FirmwareVersion": "",
                    "Id": "0-25",
                    "Links": {
                    },
                    "Manufacturer": "Intel Corporation",
                    "Model": null,
                    "Name": "Sapphire Rapids SATA AHCI Controller",
                    "PartNumber": null,
                    "SKU": null,
                    "SerialNumber": null,
                    "Slot": {},
                    "Status": {
                        "State": "Enabled",
                        "Health": "OK",
                        "HealthRollup": "OK"
                    },
                    "PCIeFunctions": {
                        "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions"
                    }
                }"##.to_string()
            },
            "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions" => {
                r#"{
                    "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions",
                    "Description": "Collection of PCIeFunctions",
                    "Members": [
                        {
                            "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions/0-24-0"
                        }
                    ],
                    "Members@odata.count": 1,
                    "Name": "PCIeFunction Collection"
                }"#.to_string()
            },
            "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions/0-24-0" => {
                r#"
                {
                    "@odata.context": "/redfish/v1/$metadata#PCIeFunction.PCIeFunction",
                    "@odata.etag": "\"1754525529\"",
                    "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions/0-24-0",
                    "ClassCode": "0x010601",
                    "Description": "Sapphire Rapids SATA AHCI Controller",
                    "DeviceClass": "MassStorageController",
                    "DeviceId": "0x1bf2",
                    "Enabled": true,
                    "FunctionId": 0,
                    "FunctionType": "Physical",
                    "Id": "0-24-0",
                    "Links": {
                    },
                    "Name": "Sapphire Rapids SATA AHCI Controller",
                    "RevisionId": "0x11",
                    "Status": {
                        "State": "Enabled",
                        "Health": "OK",
                        "HealthRollup": "OK"
                    },
                    "SubsystemId": "0x0a6b",
                    "SubsystemVendorId": "0x1028",
                    "VendorId": "0x8086"
                }"#.to_string()
            },
            "/redfish/v1/Systems" => {
                r#"{
                    "@odata.context": "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection",
                    "@odata.id": "/redfish/v1/Systems",
                    "Description": "Collection of Computer Systems",
                    "Members": [
                        {
                            "@odata.id": "/redfish/v1/Systems/System.Embedded.1"
                        }
                    ],
                    "Members@odata.count": 1,
                    "Name": "Computer System Collection"
                }"#.to_string()
            },
            "/redfish/v1/Systems/System.Embedded.1" => {
                r##"{
                    "@Redfish.Settings": {
                        "@odata.context": "/redfish/v1/$metadata#Settings.Settings",
                        "SettingsObject": {
                            "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Settings"
                        },
                        "SupportedApplyTimes": [
                            "OnReset"
                        ]
                    },
                    "@odata.context": "/redfish/v1/$metadata#ComputerSystem.ComputerSystem",
                    "@odata.id": "/redfish/v1/Systems/System.Embedded.1",
                    "Actions": {
                        "#ComputerSystem.Reset": {
                            "target": "/redfish/v1/Systems/System.Embedded.1/Actions/ComputerSystem.Reset",
                            "ResetType@Redfish.AllowableValues": [
                                "On",
                                "ForceOff",
                                "ForceRestart",
                                "GracefulRestart",
                                "GracefulShutdown",
                                "PushPowerButton",
                                "Nmi",
                                "PowerCycle"
                            ]
                        }
                    },
                    "AssetTag": "",
                    "Bios": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Bios"
                    },
                    "BiosVersion": "2.5.4",
                    "BootProgress": {
                        "LastState": "OSRunning"
                    },
                    "Boot": {
                        "BootOptions": {
                            "@odata.id": "/redfish/v1/Systems/System.Embedded.1/BootOptions"
                        },
                        "Certificates": {
                            "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Boot/Certificates"
                        },
                        "BootOrder": [
                            "Boot0000",
                            "Boot0001"
                        ],
                        "BootOrder@odata.count": 2,
                        "BootSourceOverrideEnabled": "Disabled",
                        "BootSourceOverrideMode": "UEFI",
                        "BootSourceOverrideTarget": "None",
                        "UefiTargetBootSourceOverride": null,
                        "BootSourceOverrideTarget@Redfish.AllowableValues": [
                            "None",
                            "Pxe",
                            "Floppy",
                            "Cd",
                            "Hdd",
                            "BiosSetup",
                            "Utilities",
                            "UefiTarget",
                            "SDCard",
                            "UefiHttp"
                        ],
                        "StopBootOnFault": "Never"
                    },
                    "Description": "Computer System which represents a machine (physical or virtual) and the local resources such as memory, cpu and other devices that can be accessed from that machine.",
                    "EthernetInterfaces": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/EthernetInterfaces"
                    },
                    "GraphicalConsole": {
                        "ConnectTypesSupported": [
                            "KVMIP"
                        ],
                        "ConnectTypesSupported@odata.count": 1,
                        "MaxConcurrentSessions": 6,
                        "ServiceEnabled": true
                    },
                    "HostName": "",
                    "HostWatchdogTimer": {
                        "FunctionEnabled": false,
                        "Status": {
                            "State": "Disabled"
                        },
                        "TimeoutAction": "None"
                    },
                    "HostingRoles": [],
                    "HostingRoles@odata.count": 0,
                    "Id": "System.Embedded.1",
                    "IndicatorLED": "Lit",
                    "IndicatorLED@Redfish.Deprecated": "Please migrate to use LocationIndicatorActive property",
                    "Links": {
                        "Chassis": [
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1"
                            }
                        ],
                        "Chassis@odata.count": 1,
                        "CooledBy": [
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/Thermal#/Fans/0"
                            },
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/Thermal#/Fans/1"
                            }
                        ],
                        "CooledBy@odata.count": 12,
                        "ManagedBy": [
                            {
                                "@odata.id": "/redfish/v1/Managers/iDRAC.Embedded.1"
                            }
                        ],
                        "ManagedBy@odata.count": 1,
                        "Oem": {
                        },
                        "PoweredBy": [
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/Power#/PowerSupplies/0"
                            },
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/Power#/PowerSupplies/1"
                            }
                        ],
                        "PoweredBy@odata.count": 2,
                        "TrustedComponents": [
                            {
                                "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/TrustedComponents/TPM"
                            }
                        ],
                        "TrustedComponents@odata.count": 1
                    },
                    "LastResetTime": "2025-06-16T19:47:38-05:00",
                    "LocationIndicatorActive": false,
                    "Manufacturer": "Dell Inc.",
                    "Memory": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Memory"
                    },
                    "MemorySummary": {
                        "MemoryMirroring": "System",
                        "Status": {
                            "Health": "OK",
                            "HealthRollup": "OK",
                            "State": "Enabled"
                        },
                        "Status@Redfish.Deprecated": "Please migrate to use Status in the individual Memory resources",
                        "TotalSystemMemoryGiB": 256
                    },
                    "Model": "PowerEdge R760",
                    "Name": "System",
                    "NetworkInterfaces": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/NetworkInterfaces"
                    },
                    "Oem": {
                    },
                    "PCIeDevices": [
                        {
                            "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24"
                        },
                        {
                            "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/3-0"
                        }
                    ],
                    "PCIeDevices@odata.count": 9,
                    "PCIeFunctions": [
                        {
                            "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/0-24/PCIeFunctions/0-24-0"
                        },
                        {
                            "@odata.id": "/redfish/v1/Chassis/System.Embedded.1/PCIeDevices/3-0/PCIeFunctions/3-0-0"
                        }
                    ],
                    "PCIeFunctions@odata.count": 12,
                    "PartNumber": "0C9W19A03",
                    "PowerState": "On",
                    "ProcessorSummary": {
                        "Count": 2,
                        "CoreCount": 48,
                        "LogicalProcessorCount": 96,
                        "Model": "Intel(R) Xeon(R) Gold 6442Y",
                        "Status": {
                            "Health": "OK",
                            "HealthRollup": "OK",
                            "State": "Enabled"
                        },
                        "Status@Redfish.Deprecated": "Please migrate to use Status in the individual Processor resources",
                        "ThreadingEnabled": true
                    },
                    "Processors": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Processors"
                    },
                    "SKU": "5D68144",
                    "SecureBoot": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/SecureBoot"
                    },
                    "SerialNumber": "MXWSJ0045G00VJ",
                    "SimpleStorage": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/SimpleStorage"
                    },
                    "Status": {
                        "Health": "OK",
                        "HealthRollup": "OK",
                        "State": "Enabled"
                    },
                    "Storage": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/Storage"
                    },
                    "SystemType": "Physical",
                    "TrustedModules": [
                        {
                            "FirmwareVersion": "7.2.3.1",
                            "InterfaceType": "TPM2_0",
                            "Status": {
                                "State": "Enabled"
                            }
                        }
                    ],
                    "TrustedModules@odata.count": 1,
                    "UUID": "4c4c4544-0044-3610-8038-b5c04f313434",
                    "VirtualMedia": {
                        "@odata.id": "/redfish/v1/Systems/System.Embedded.1/VirtualMedia"
                    },
                    "VirtualMediaConfig": {
                        "ServiceEnabled": true
                    }
                }"##.to_string()
            }
            "/redfish/v1/Oem/Contoso/TurboencabulatorService" => {
                r##"{
                       "@odata.id": "/redfish/v1/Oem/Contoso/TurboencabulatorService",
                       "Id": "TurboencabulatorService",
                       "Name": "Turboencabulator Service",
                       "Status": {
                           "State": "Enabled",
                           "Health": "OK"
                       },
                       "IsCheap": false,
                       "ServiceEnabled": true,
                       "TurboencabulatorMode": "Retro",
                       "WillGovernmentBuy": true
                   }"##.to_string()
            }
            _ => {
                r#"{"id": "unknown", "name": "Unknown Resource"}"#.to_string()
            }
        }
    }
}

impl Bmc for MockBmc {
    type Error = Error;

    async fn expand<T>(&self, _id: &ODataId, _query: ExpandQuery) -> Result<Arc<T>, Error>
    where
        T: Expandable,
    {
        todo!("unimplimented")
    }

    async fn get<T: nv_redfish::EntityType + Sized + for<'a> serde::Deserialize<'a>>(
        &self,
        id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        println!("BMC GET {id}");
        // In real implementation: async HTTP GET request and JSON deserialization
        let mock_json = self.get_mock_json_for_uri(&id.to_string());
        let result: T = serde_json::from_str(&mock_json).map_err(Error::ParseError)?;
        Ok(Arc::new(result))
    }

    async fn update<V: Sync + Send + Serialize>(
        &self,
        _id: &ODataId,
        _v: &V,
    ) -> Result<(), Self::Error> {
        todo!("unimplimented")
    }

    async fn create<
        V: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        _id: &ODataId,
        _v: &V,
    ) -> Result<R, Self::Error> {
        todo!("unimplimented")
    }

    async fn delete(&self, _id: &ODataId) -> Result<(), Self::Error> {
        todo!("unimplimented")
    }

    async fn action<
        T: Send + Sync + serde::Serialize,
        R: Send + Sync + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        _action: &nv_redfish::Action<T, R>,
        _params: &T,
    ) -> Result<R, Self::Error> {
        todo!()
    }
}

#[derive(Deserialize, Debug)]
struct Constoso {
    #[serde(rename = "Contoso")]
    oem_root: redfish_oem_contoso::redfish::contoso_service_root::ServiceRoot,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let bmc = MockBmc::default();

    let service_root = bmc.get_service_root().await?;

    let chassis_members = &service_root
        .chassis
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;

    let chassis = chassis_members.iter().next().unwrap().get(&bmc).await?;

    let all_devices = &chassis
        .pc_ie_devices
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;
    for device in all_devices {
        let function_handles = &device
            .get(&bmc)
            .await?
            .pc_ie_functions
            .as_ref()
            .unwrap()
            .get(&bmc)
            .await?
            .members;
        for function_handle in function_handles {
            let _function = function_handle.get(&bmc).await?;
        }
    }

    let systems = &service_root
        .systems
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;
    println!("{:#?}", systems.iter().next().unwrap().get(&bmc).await?);

    // Oem:
    let contoso_oem: Constoso = serde_json::from_value(
        service_root
            .base
            .oem
            .as_ref()
            .unwrap()
            .additional_properties
            .clone(),
    )
    .unwrap();

    let turboencabulator_service = contoso_oem
        .oem_root
        .turboencabulator_service
        .unwrap()
        .get(&bmc)
        .await?;
    println!("{:#?}", turboencabulator_service);

    Ok(())
}
