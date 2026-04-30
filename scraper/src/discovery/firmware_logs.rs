// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::collections::VecDeque;

use nv_redfish::schema::chassis::Chassis;
use nv_redfish::schema::chassis_collection::ChassisCollection;
use nv_redfish::schema::computer_system::ComputerSystem;
use nv_redfish::schema::computer_system_collection::ComputerSystemCollection;
use nv_redfish::schema::log_service_collection::LogServiceCollection;
use nv_redfish::schema::manager::Manager;
use nv_redfish::schema::manager_collection::ManagerCollection;
use nv_redfish::schema::service_root::ServiceRoot;
use nv_redfish::schema::software_inventory_collection::SoftwareInventoryCollection;
use nv_redfish::schema::update_service::UpdateService;
use nv_redfish_core::Bmc;
use nv_redfish_core::EntityTypeRef as _;
use nv_redfish_core::ODataId;

use super::DiscoveryBatch;
use super::TypedDiscoveryContext;
use crate::Error;

pub(super) async fn discover_firmware<B>(
    cx: &TypedDiscoveryContext<'_, B>,
) -> Result<DiscoveryBatch, Error>
where
    B: Bmc,
    B::Error: 'static,
{
    StandardFirmwareDiscovery::new().discover(cx).await
}

struct StandardFirmwareDiscovery {
    pending: VecDeque<StandardFirmwareWork>,
    candidates: BTreeSet<ODataId>,
}

impl StandardFirmwareDiscovery {
    fn new() -> Self {
        Self {
            pending: VecDeque::from([StandardFirmwareWork::ServiceRoot]),
            candidates: BTreeSet::new(),
        }
    }

    async fn discover<B>(
        mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<DiscoveryBatch, Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        while let Some(work) = self.pending.pop_front() {
            self.advance(cx, work).await?;
        }
        Ok(DiscoveryBatch::candidates(self.candidates))
    }

    async fn advance<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        work: StandardFirmwareWork,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        match work {
            StandardFirmwareWork::ServiceRoot => self.fetch_service_root(cx).await,
            StandardFirmwareWork::UpdateService(id) => self.fetch_update_service(cx, id).await,
            StandardFirmwareWork::SoftwareInventoryCollection(id) => {
                self.fetch_software_inventory_collection(cx, id).await
            }
        }
    }

    async fn fetch_service_root<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let root = cx.fetch::<ServiceRoot>(ODataId::service_root()).await?;
        if let Some(update_service_ref) = &root.update_service {
            self.pending.push_back(StandardFirmwareWork::UpdateService(
                update_service_ref.odata_id().clone(),
            ));
        }
        Ok(())
    }

    async fn fetch_update_service<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let update_service = cx.fetch::<UpdateService>(id).await?;
        if let Some(collection_ref) = &update_service.firmware_inventory {
            self.pending
                .push_back(StandardFirmwareWork::SoftwareInventoryCollection(
                    collection_ref.odata_id().clone(),
                ));
        }
        if let Some(collection_ref) = &update_service.software_inventory {
            self.pending
                .push_back(StandardFirmwareWork::SoftwareInventoryCollection(
                    collection_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_software_inventory_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let collection = cx.fetch::<SoftwareInventoryCollection>(id).await?;
        self.candidates.extend(
            collection
                .members
                .iter()
                .map(|member| member.odata_id().clone()),
        );
        Ok(())
    }
}

enum StandardFirmwareWork {
    ServiceRoot,
    UpdateService(ODataId),
    SoftwareInventoryCollection(ODataId),
}

pub(super) async fn discover_log_services<B>(
    cx: &TypedDiscoveryContext<'_, B>,
) -> Result<DiscoveryBatch, Error>
where
    B: Bmc,
    B::Error: 'static,
{
    StandardLogServiceDiscovery::new().discover(cx).await
}

struct StandardLogServiceDiscovery {
    pending: VecDeque<StandardLogServiceWork>,
    candidates: BTreeSet<ODataId>,
}

impl StandardLogServiceDiscovery {
    fn new() -> Self {
        Self {
            pending: VecDeque::from([StandardLogServiceWork::ServiceRoot]),
            candidates: BTreeSet::new(),
        }
    }

    async fn discover<B>(
        mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<DiscoveryBatch, Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        while let Some(work) = self.pending.pop_front() {
            self.advance(cx, work).await?;
        }
        Ok(DiscoveryBatch::candidates(self.candidates))
    }

    async fn advance<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        work: StandardLogServiceWork,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        match work {
            StandardLogServiceWork::ServiceRoot => self.fetch_service_root(cx).await,
            StandardLogServiceWork::ChassisCollection(id) => {
                self.fetch_chassis_collection(cx, id).await
            }
            StandardLogServiceWork::Chassis(id) => self.fetch_chassis(cx, id).await,
            StandardLogServiceWork::SystemCollection(id) => {
                self.fetch_system_collection(cx, id).await
            }
            StandardLogServiceWork::System(id) => self.fetch_system(cx, id).await,
            StandardLogServiceWork::ManagerCollection(id) => {
                self.fetch_manager_collection(cx, id).await
            }
            StandardLogServiceWork::Manager(id) => self.fetch_manager(cx, id).await,
            StandardLogServiceWork::LogServiceCollection(id) => {
                self.fetch_log_service_collection(cx, id).await
            }
        }
    }

    async fn fetch_service_root<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let root = cx.fetch::<ServiceRoot>(ODataId::service_root()).await?;
        if let Some(chassis_collection_ref) = &root.chassis {
            self.pending
                .push_back(StandardLogServiceWork::ChassisCollection(
                    chassis_collection_ref.odata_id().clone(),
                ));
        }
        if let Some(system_collection_ref) = &root.systems {
            self.pending
                .push_back(StandardLogServiceWork::SystemCollection(
                    system_collection_ref.odata_id().clone(),
                ));
        }
        if let Some(manager_collection_ref) = &root.managers {
            self.pending
                .push_back(StandardLogServiceWork::ManagerCollection(
                    manager_collection_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_chassis_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let collection = cx.fetch::<ChassisCollection>(id).await?;
        self.pending.extend(
            collection
                .members
                .iter()
                .map(|member| StandardLogServiceWork::Chassis(member.odata_id().clone())),
        );
        Ok(())
    }

    async fn fetch_chassis<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let chassis = cx.fetch::<Chassis>(id).await?;
        if let Some(log_services_ref) = &chassis.log_services {
            self.pending
                .push_back(StandardLogServiceWork::LogServiceCollection(
                    log_services_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_system_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let collection = cx.fetch::<ComputerSystemCollection>(id).await?;
        self.pending.extend(
            collection
                .members
                .iter()
                .map(|member| StandardLogServiceWork::System(member.odata_id().clone())),
        );
        Ok(())
    }

    async fn fetch_system<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let system = cx.fetch::<ComputerSystem>(id).await?;
        if let Some(log_services_ref) = &system.log_services {
            self.pending
                .push_back(StandardLogServiceWork::LogServiceCollection(
                    log_services_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_manager_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let collection = cx.fetch::<ManagerCollection>(id).await?;
        self.pending.extend(
            collection
                .members
                .iter()
                .map(|member| StandardLogServiceWork::Manager(member.odata_id().clone())),
        );
        Ok(())
    }

    async fn fetch_manager<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let manager = cx.fetch::<Manager>(id).await?;
        if let Some(log_services_ref) = &manager.log_services {
            self.pending
                .push_back(StandardLogServiceWork::LogServiceCollection(
                    log_services_ref.odata_id().clone(),
                ));
        }
        Ok(())
    }

    async fn fetch_log_service_collection<B>(
        &mut self,
        cx: &TypedDiscoveryContext<'_, B>,
        id: ODataId,
    ) -> Result<(), Error>
    where
        B: Bmc,
        B::Error: 'static,
    {
        let collection = cx.fetch::<LogServiceCollection>(id).await?;
        self.candidates.extend(
            collection
                .members
                .iter()
                .map(|member| member.odata_id().clone()),
        );
        Ok(())
    }
}

enum StandardLogServiceWork {
    ServiceRoot,
    ChassisCollection(ODataId),
    Chassis(ODataId),
    SystemCollection(ODataId),
    System(ODataId),
    ManagerCollection(ODataId),
    Manager(ODataId),
    LogServiceCollection(ODataId),
}
