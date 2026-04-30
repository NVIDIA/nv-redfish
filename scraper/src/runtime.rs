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

use std::{collections::HashMap, collections::VecDeque, convert::Infallible, time::Instant};

use crate::{
    scheduler::FlatRoundRobin, Generator, GeneratorId, RuntimeOutput, ScheduledWork, TargetId,
    WorkCompletion, WorkError, WorkOutcome, WorkSuccess,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TargetConfig {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddGeneratorError {
    TargetNotFound { target_id: TargetId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOnce {
    Executed,
    Idle,
}

pub struct Runtime<'rt, E, Err, R = Infallible> {
    inner: RuntimeInner<'rt, E, Err, R>,
}

impl<'rt, E, Err, R> Runtime<'rt, E, Err, R> {
    pub fn new() -> Self {
        Self {
            inner: RuntimeInner::new(),
        }
    }

    pub fn add_target(&mut self, _config: TargetConfig) -> TargetId {
        self.inner.add_target()
    }

    pub fn remove_target(&mut self, target_id: TargetId) -> bool {
        self.inner.remove_target(target_id)
    }

    pub fn add_generator<G>(
        &mut self,
        target_id: TargetId,
        generator: G,
    ) -> Result<GeneratorId, AddGeneratorError>
    where
        G: Generator<'rt, E, Err> + 'rt,
    {
        self.inner.add_generator(target_id, generator)
    }

    pub fn remove_generator(&mut self, generator_id: GeneratorId) -> bool {
        self.inner.remove_generator(generator_id)
    }

    pub async fn run_once(&mut self) -> RunOnce {
        match self.inner.select_work() {
            Some((generator_id, work)) => {
                let result = match work.execute().await {
                    Ok(events) => {
                        let success = WorkSuccess {
                            generator_id,
                            events,
                        };
                        let completion = WorkCompletion {
                            generator_id,
                            outcome: WorkOutcome::Succeeded,
                        };
                        self.inner
                            .outputs
                            .push_back(RuntimeOutput::Work(Ok(success)));
                        completion
                    }
                    Err(error) => {
                        let work_error = WorkError {
                            generator_id,
                            error,
                        };
                        let completion = WorkCompletion {
                            generator_id,
                            outcome: WorkOutcome::Failed,
                        };
                        self.inner
                            .outputs
                            .push_back(RuntimeOutput::Work(Err(work_error)));
                        completion
                    }
                };
                self.inner.complete_work(generator_id, &result);
                RunOnce::Executed
            }
            None => RunOnce::Idle,
        }
    }

    pub fn next_output(&mut self) -> Option<RuntimeOutput<E, Err, R>> {
        self.inner.outputs.pop_front()
    }

    pub fn drain_outputs(&mut self) -> Vec<RuntimeOutput<E, Err, R>> {
        self.inner.outputs.drain(..).collect::<Vec<_>>()
    }
}

impl<'rt, E, Err, R> Default for Runtime<'rt, E, Err, R> {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimeInner<'rt, E, Err, R> {
    next_target_id: u64,
    targets: HashMap<TargetId, TargetState>,
    generators: HashMap<GeneratorId, GeneratorSlot<'rt, E, Err>>,
    scheduler: FlatRoundRobin,
    outputs: VecDeque<RuntimeOutput<E, Err, R>>,
}

impl<'rt, E, Err, R> RuntimeInner<'rt, E, Err, R> {
    fn new() -> Self {
        Self {
            next_target_id: 1,
            targets: HashMap::new(),
            generators: HashMap::new(),
            scheduler: FlatRoundRobin::default(),
            outputs: VecDeque::new(),
        }
    }

    fn add_target(&mut self) -> TargetId {
        let target_id = TargetId::new(self.next_target_id);
        self.next_target_id += 1;
        self.targets.insert(target_id, TargetState::new());
        target_id
    }

    fn remove_target(&mut self, target_id: TargetId) -> bool {
        self.targets
            .remove(&target_id)
            .map(|target| {
                target.generators.into_iter().for_each(|generator_id| {
                    self.generators.remove(&generator_id);
                    self.scheduler.remove(generator_id);
                });
            })
            .is_some()
    }

    fn add_generator<G>(
        &mut self,
        target_id: TargetId,
        generator: G,
    ) -> Result<GeneratorId, AddGeneratorError>
    where
        G: Generator<'rt, E, Err> + 'rt,
    {
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or(AddGeneratorError::TargetNotFound { target_id })?;
        let generator_id = target.next_generator_id(target_id);
        target.generators.push(generator_id);
        self.generators.insert(
            generator_id,
            GeneratorSlot {
                generator: Box::new(generator),
            },
        );
        self.scheduler.insert(generator_id);
        Ok(generator_id)
    }

    fn remove_generator(&mut self, generator_id: GeneratorId) -> bool {
        self.generators
            .remove(&generator_id)
            .map(|_| {
                if let Some(target) = self.targets.get_mut(&generator_id.target_id()) {
                    target.remove_generator(generator_id);
                }
                self.scheduler.remove(generator_id);
            })
            .is_some()
    }

    fn select_work(&mut self) -> Option<(GeneratorId, ScheduledWork<'rt, E, Err>)> {
        let now = Instant::now();
        let scheduler = &mut self.scheduler;
        let generators = &mut self.generators;
        scheduler.find_map(|generator_id| {
            generators.get_mut(&generator_id).and_then(|slot| {
                slot.generator
                    .update_ready(now)
                    .ready
                    .then(|| slot.generator.take_next())
                    .flatten()
                    .map(|work| (generator_id, work))
            })
        })
    }

    fn complete_work(&mut self, generator_id: GeneratorId, completion: &WorkCompletion) {
        if let Some(slot) = self.generators.get_mut(&generator_id) {
            slot.generator.on_complete(completion);
        }
    }
}

struct TargetState {
    next_generator_id: u64,
    generators: Vec<GeneratorId>,
}

impl TargetState {
    fn new() -> Self {
        Self {
            next_generator_id: 1,
            generators: Vec::new(),
        }
    }

    fn next_generator_id(&mut self, target_id: TargetId) -> GeneratorId {
        let generator_id = GeneratorId::new(target_id, self.next_generator_id);
        self.next_generator_id += 1;
        generator_id
    }

    fn remove_generator(&mut self, generator_id: GeneratorId) {
        self.generators
            .retain(|candidate| *candidate != generator_id);
    }
}

struct GeneratorSlot<'rt, E, Err> {
    generator: Box<dyn Generator<'rt, E, Err> + 'rt>,
}
