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

use crate::Expect;
use nv_redfish::ActionError;
use nv_redfish::Bmc as NvRedfishBmc;
use nv_redfish::Expandable;
use nv_redfish::ODataId;
use nv_redfish::http::ExpandQuery;
use serde::Serialize;
use serde_json::Error as JsonError;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

#[derive(Debug)]
pub enum Error {
    NotSupported,
    MutexLock(String),
    NothingIsExpected,
    BadResponseJson(JsonError),
    UnexpectedGet(ODataId, Expect),
}

impl Error {
    pub fn mutex_lock<T>(err: PoisonError<T>) -> Self {
        Self::MutexLock(err.to_string())
    }
}

#[derive(Default)]
pub struct Bmc {
    expect: Mutex<VecDeque<Expect>>,
}

impl Bmc {
    pub fn expect(&self, exp: Expect) {
        let expect: &mut VecDeque<Expect> = &mut self.expect.lock().expect("not poisoned");
        expect.clear();
        expect.push_front(exp);
    }

    pub fn debug_expect(&self) {
        let expect: &VecDeque<Expect> = &self.expect.lock().expect("not poisoned");
        println!("Expectations (total: {})", expect.len());
        for v in expect {
            println!("{v:#?}");
        }
    }
}

impl NvRedfishBmc for Bmc {
    type Error = Error;

    async fn expand<T>(&self, _id: &ODataId, _query: ExpandQuery) -> Result<Arc<T>, Error>
    where
        T: Expandable,
    {
        todo!("unimplimented")
    }

    async fn get<T: nv_redfish::EntityType + Sized + for<'a> serde::Deserialize<'a>>(
        &self,
        in_id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        match expect {
            Expect::Get { id, response } if id == *in_id => {
                let result: T = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(Arc::new(result))
            }
            _ => Err(Error::UnexpectedGet(in_id.clone(), expect)),
        }
    }

    async fn update<
        V: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        _id: &ODataId,
        _update: &V,
    ) -> Result<R, Self::Error> {
        todo!("unimplimented")
    }

    async fn create<
        V: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        _id: &ODataId,
        _create: &V,
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
        todo!("unimplimented")
    }
}

impl ActionError for Error {
    fn not_supported() -> Self {
        Error::NotSupported
    }
}
