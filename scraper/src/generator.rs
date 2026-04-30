// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use std::{future::Future, pin::Pin, time::Instant};

use crate::WorkCompletion;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Readiness {
    pub ready: bool,
    pub next_ready_at: Option<Instant>,
}

pub trait Generator<'rt, E, Err>: Send {
    fn update_ready(&mut self, now: Instant) -> Readiness;
    fn take_next(&mut self) -> Option<ScheduledWork<'rt, E, Err>>;
    fn on_complete(&mut self, completion: &WorkCompletion);
}

pub struct ScheduledWork<'rt, E, Err> {
    future: Pin<Box<dyn Future<Output = Result<Vec<E>, Err>> + Send + 'rt>>,
}

impl<'rt, E, Err> ScheduledWork<'rt, E, Err> {
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Output = Result<Vec<E>, Err>> + Send + 'rt,
    {
        Self {
            future: Box::pin(future),
        }
    }

    pub(crate) async fn execute(self) -> Result<Vec<E>, Err> {
        self.future.await
    }
}
