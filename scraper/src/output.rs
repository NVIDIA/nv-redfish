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

use crate::GeneratorId;

pub type WorkResult<E, Err> = Result<WorkSuccess<E>, WorkError<Err>>;

pub struct WorkSuccess<E> {
    pub generator_id: GeneratorId,
    pub events: Vec<E>,
}

pub struct WorkError<Err> {
    pub generator_id: GeneratorId,
    pub error: Err,
}

pub enum RuntimeOutput<E, Err, R = std::convert::Infallible> {
    Work(WorkResult<E, Err>),
    Runtime(R),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkCompletion {
    pub generator_id: GeneratorId,
    pub outcome: WorkOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkOutcome {
    Succeeded,
    Failed,
}
