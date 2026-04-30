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

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

use nv_redfish_scraper::{
    AddGeneratorError, Generator, Readiness, RunOnce, Runtime, RuntimeOutput, ScheduledWork,
    TargetConfig, WorkCompletion, WorkOutcome,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum FakeExplorerEvent {
    Label(&'static str),
    ServiceRootDiscovered {
        systems: Vec<String>,
        chassis: Vec<String>,
    },
    SystemDiscovered {
        system_id: String,
    },
    ChassisDiscovered {
        chassis_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeExplorerError(&'static str);

type FakeRuntime = Runtime<'static, FakeExplorerEvent, FakeExplorerError>;

struct FakeGenerator {
    ready: bool,
    outcomes: VecDeque<Result<Vec<FakeExplorerEvent>, FakeExplorerError>>,
    readiness_calls: Arc<Mutex<usize>>,
    take_next_calls: Arc<Mutex<usize>>,
    completions: Arc<Mutex<Vec<WorkCompletion>>>,
}

impl FakeGenerator {
    fn with_outcomes(outcomes: Vec<Result<Vec<FakeExplorerEvent>, FakeExplorerError>>) -> Self {
        Self {
            ready: true,
            outcomes: outcomes.into(),
            readiness_calls: Arc::new(Mutex::new(0)),
            take_next_calls: Arc::new(Mutex::new(0)),
            completions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn not_ready() -> Self {
        Self {
            ready: false,
            outcomes: VecDeque::new(),
            readiness_calls: Arc::new(Mutex::new(0)),
            take_next_calls: Arc::new(Mutex::new(0)),
            completions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn handles(&self) -> FakeGeneratorHandles {
        FakeGeneratorHandles {
            readiness_calls: Arc::clone(&self.readiness_calls),
            take_next_calls: Arc::clone(&self.take_next_calls),
            completions: Arc::clone(&self.completions),
        }
    }
}

impl Generator<'static, FakeExplorerEvent, FakeExplorerError> for FakeGenerator {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        *self
            .readiness_calls
            .lock()
            .expect("readiness lock poisoned") += 1;
        Readiness {
            ready: self.ready,
            next_ready_at: None,
        }
    }

    fn take_next(
        &mut self,
    ) -> Option<ScheduledWork<'static, FakeExplorerEvent, FakeExplorerError>> {
        *self
            .take_next_calls
            .lock()
            .expect("take-next lock poisoned") += 1;
        self.outcomes
            .pop_front()
            .map(|outcome| ScheduledWork::new(async move { outcome }))
    }

    fn on_complete(&mut self, completion: &WorkCompletion) {
        self.completions
            .lock()
            .expect("completion lock poisoned")
            .push(*completion);
    }
}

#[derive(Clone)]
struct FakeGeneratorHandles {
    readiness_calls: Arc<Mutex<usize>>,
    take_next_calls: Arc<Mutex<usize>>,
    completions: Arc<Mutex<Vec<WorkCompletion>>>,
}

impl FakeGeneratorHandles {
    fn readiness_calls(&self) -> usize {
        *self
            .readiness_calls
            .lock()
            .expect("readiness lock poisoned")
    }

    fn take_next_calls(&self) -> usize {
        *self
            .take_next_calls
            .lock()
            .expect("take-next lock poisoned")
    }

    fn completions(&self) -> Vec<WorkCompletion> {
        self.completions
            .lock()
            .expect("completion lock poisoned")
            .clone()
    }
}

fn one_event(label: &'static str) -> FakeGenerator {
    FakeGenerator::with_outcomes(vec![Ok(vec![FakeExplorerEvent::Label(label)])])
}

fn repeating(label: &'static str, count: usize) -> FakeGenerator {
    FakeGenerator::with_outcomes(
        (0..count)
            .map(|_| Ok(vec![FakeExplorerEvent::Label(label)]))
            .collect::<Vec<_>>(),
    )
}

fn labels(outputs: Vec<RuntimeOutput<FakeExplorerEvent, FakeExplorerError>>) -> Vec<&'static str> {
    outputs
        .into_iter()
        .flat_map(|output| match output {
            RuntimeOutput::Work(Ok(success)) => success.events,
            RuntimeOutput::Work(Err(_)) => Vec::new(),
            RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
        })
        .filter_map(|event| match event {
            FakeExplorerEvent::Label(label) => Some(label),
            _ => None,
        })
        .collect::<Vec<_>>()
}

#[tokio::test]
async fn target_ids_are_generated() {
    let mut runtime = FakeRuntime::new();

    let first = runtime.add_target(TargetConfig {});
    let second = runtime.add_target(TargetConfig {});

    assert_ne!(first, second);
    assert_eq!(first.to_string(), "target #1");
    assert_eq!(second.to_string(), "target #2");
}

#[tokio::test]
async fn generator_ids_include_target_ids() {
    let mut runtime = FakeRuntime::new();
    let first_target = runtime.add_target(TargetConfig {});
    let second_target = runtime.add_target(TargetConfig {});

    let first = runtime.add_generator(first_target, one_event("a")).unwrap();
    let second = runtime.add_generator(first_target, one_event("b")).unwrap();
    let third = runtime
        .add_generator(second_target, one_event("c"))
        .unwrap();

    assert_eq!(first.target_id(), first_target);
    assert_eq!(second.target_id(), first_target);
    assert_eq!(third.target_id(), second_target);
    assert_eq!(first.to_string(), "generator #1.1");
    assert_eq!(second.to_string(), "generator #1.2");
    assert_eq!(third.to_string(), "generator #2.1");
}

#[tokio::test]
async fn add_generator_requires_existing_target() {
    let mut runtime = FakeRuntime::new();
    let removed = runtime.add_target(TargetConfig {});
    assert!(runtime.remove_target(removed));

    let error = runtime.add_generator(removed, one_event("a")).unwrap_err();

    assert_eq!(
        error,
        AddGeneratorError::TargetNotFound { target_id: removed }
    );
}

#[tokio::test]
async fn flat_round_robin_order() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    runtime.add_generator(target, repeating("A", 2)).unwrap();
    runtime.add_generator(target, repeating("B", 2)).unwrap();
    runtime.add_generator(target, repeating("C", 2)).unwrap();

    for _ in 0..6 {
        assert_eq!(runtime.run_once().await, RunOnce::Executed);
    }

    assert_eq!(
        labels(runtime.drain_outputs()),
        vec!["A", "B", "C", "A", "B", "C"]
    );
}

#[tokio::test]
async fn output_event_ordering_inside_one_work_item() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    runtime
        .add_generator(
            target,
            FakeGenerator::with_outcomes(vec![Ok(vec![
                FakeExplorerEvent::Label("first"),
                FakeExplorerEvent::Label("second"),
                FakeExplorerEvent::Label("third"),
            ])]),
        )
        .unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    assert_eq!(
        labels(runtime.drain_outputs()),
        vec!["first", "second", "third"]
    );
}

#[tokio::test]
async fn work_error_output() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let generator_id = runtime
        .add_generator(
            target,
            FakeGenerator::with_outcomes(vec![Err(FakeExplorerError("boom"))]),
        )
        .unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    let outputs = runtime.drain_outputs();
    assert_eq!(outputs.len(), 1);
    match outputs.into_iter().next().unwrap() {
        RuntimeOutput::Work(Err(error)) => {
            assert_eq!(error.generator_id, generator_id);
            assert_eq!(error.error, FakeExplorerError("boom"));
        }
        RuntimeOutput::Work(Ok(_)) => panic!("expected work error"),
        RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
    }
}

#[tokio::test]
async fn completion_callback_on_success() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let generator = one_event("ok");
    let handles = generator.handles();
    let generator_id = runtime.add_generator(target, generator).unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    assert_eq!(
        handles.completions(),
        vec![WorkCompletion {
            generator_id,
            outcome: WorkOutcome::Succeeded,
        }]
    );
}

#[tokio::test]
async fn completion_callback_on_failure() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let generator = FakeGenerator::with_outcomes(vec![Err(FakeExplorerError("boom"))]);
    let handles = generator.handles();
    let generator_id = runtime.add_generator(target, generator).unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    assert_eq!(
        handles.completions(),
        vec![WorkCompletion {
            generator_id,
            outcome: WorkOutcome::Failed,
        }]
    );
}

#[tokio::test]
async fn not_ready_generator_is_skipped() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let not_ready = FakeGenerator::not_ready();
    let not_ready_handles = not_ready.handles();
    let ready = one_event("ready");
    let ready_handles = ready.handles();

    runtime.add_generator(target, not_ready).unwrap();
    runtime.add_generator(target, ready).unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    assert_eq!(not_ready_handles.readiness_calls(), 1);
    assert_eq!(not_ready_handles.take_next_calls(), 0);
    assert_eq!(ready_handles.take_next_calls(), 1);
    assert_eq!(labels(runtime.drain_outputs()), vec!["ready"]);
}

#[tokio::test]
async fn work_is_created_only_when_selected() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let first = one_event("first");
    let first_handles = first.handles();
    let second = one_event("second");
    let second_handles = second.handles();

    runtime.add_generator(target, first).unwrap();
    runtime.add_generator(target, second).unwrap();

    assert_eq!(first_handles.take_next_calls(), 0);
    assert_eq!(second_handles.take_next_calls(), 0);
    assert_eq!(runtime.run_once().await, RunOnce::Executed);
    assert_eq!(first_handles.take_next_calls(), 1);
    assert_eq!(second_handles.take_next_calls(), 0);
}

#[tokio::test]
async fn remove_generator() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let first = repeating("A", 2);
    let first_handles = first.handles();
    let second = repeating("B", 2);
    let second_handles = second.handles();
    runtime.add_generator(target, first).unwrap();
    let second_id = runtime.add_generator(target, second).unwrap();

    assert!(runtime.remove_generator(second_id));
    assert!(!runtime.remove_generator(second_id));
    assert_eq!(runtime.run_once().await, RunOnce::Executed);
    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    assert!(first_handles.readiness_calls() >= 2);
    assert_eq!(second_handles.readiness_calls(), 0);
    assert_eq!(second_handles.take_next_calls(), 0);
    assert_eq!(labels(runtime.drain_outputs()), vec!["A", "A"]);
}

#[tokio::test]
async fn remove_target_removes_generators() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let first = one_event("A");
    let first_handles = first.handles();
    let second = one_event("B");
    let second_handles = second.handles();
    runtime.add_generator(target, first).unwrap();
    runtime.add_generator(target, second).unwrap();

    assert!(runtime.remove_target(target));
    assert!(!runtime.remove_target(target));
    assert_eq!(runtime.run_once().await, RunOnce::Idle);

    assert_eq!(first_handles.readiness_calls(), 0);
    assert_eq!(second_handles.readiness_calls(), 0);
}

#[tokio::test]
async fn produced_outputs_survive_removal() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    let generator_id = runtime
        .add_generator(target, one_event("survives"))
        .unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);
    assert!(runtime.remove_generator(generator_id));

    assert_eq!(labels(runtime.drain_outputs()), vec!["survives"]);
}

#[tokio::test]
async fn bmc_explorer_like_discovery_flow() {
    let mut runtime = FakeRuntime::new();
    let target = runtime.add_target(TargetConfig {});
    runtime
        .add_generator(
            target,
            FakeGenerator::with_outcomes(vec![Ok(vec![
                FakeExplorerEvent::ServiceRootDiscovered {
                    systems: vec!["system-1".to_string(), "system-2".to_string()],
                    chassis: vec!["chassis-1".to_string()],
                },
            ])]),
        )
        .unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    let discovered = runtime
        .drain_outputs()
        .into_iter()
        .flat_map(|output| match output {
            RuntimeOutput::Work(Ok(success)) => success.events,
            RuntimeOutput::Work(Err(_)) => Vec::new(),
            RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
        })
        .filter_map(|event| match event {
            FakeExplorerEvent::ServiceRootDiscovered { systems, chassis } => {
                Some((systems, chassis))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    discovered.iter().for_each(|(systems, chassis)| {
        systems.iter().for_each(|system_id| {
            runtime
                .add_generator(
                    target,
                    FakeGenerator::with_outcomes(vec![Ok(vec![
                        FakeExplorerEvent::SystemDiscovered {
                            system_id: system_id.clone(),
                        },
                    ])]),
                )
                .unwrap();
        });
        chassis.iter().for_each(|chassis_id| {
            runtime
                .add_generator(
                    target,
                    FakeGenerator::with_outcomes(vec![Ok(vec![
                        FakeExplorerEvent::ChassisDiscovered {
                            chassis_id: chassis_id.clone(),
                        },
                    ])]),
                )
                .unwrap();
        });
    });

    assert_eq!(runtime.run_once().await, RunOnce::Executed);
    assert_eq!(runtime.run_once().await, RunOnce::Executed);
    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    let report = runtime
        .drain_outputs()
        .into_iter()
        .flat_map(|output| match output {
            RuntimeOutput::Work(Ok(success)) => success.events,
            RuntimeOutput::Work(Err(_)) => Vec::new(),
            RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
        })
        .map(|event| match event {
            FakeExplorerEvent::SystemDiscovered { system_id } => format!("system:{system_id}"),
            FakeExplorerEvent::ChassisDiscovered { chassis_id } => format!("chassis:{chassis_id}"),
            FakeExplorerEvent::ServiceRootDiscovered { .. } | FakeExplorerEvent::Label(_) => {
                "unexpected".to_string()
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        report,
        vec!["system:system-1", "system:system-2", "chassis:chassis-1"]
    );
}

#[tokio::test]
async fn payloads_do_not_need_clone_debug_or_partial_eq() {
    struct EventPayload(String);
    struct ErrorPayload(String);
    struct SingleUseGenerator;

    impl Generator<'static, EventPayload, ErrorPayload> for SingleUseGenerator {
        fn update_ready(&mut self, _now: Instant) -> Readiness {
            Readiness {
                ready: true,
                next_ready_at: None,
            }
        }

        fn take_next(&mut self) -> Option<ScheduledWork<'static, EventPayload, ErrorPayload>> {
            Some(ScheduledWork::new(async {
                Ok(vec![EventPayload("event".to_string())])
            }))
        }

        fn on_complete(&mut self, _completion: &WorkCompletion) {}
    }

    let mut runtime = Runtime::<'static, EventPayload, ErrorPayload>::new();
    let target = runtime.add_target(TargetConfig {});
    let generator_id = runtime.add_generator(target, SingleUseGenerator).unwrap();

    assert_eq!(runtime.run_once().await, RunOnce::Executed);

    match runtime.next_output().unwrap() {
        RuntimeOutput::Work(Ok(success)) => {
            assert_eq!(success.generator_id, generator_id);
            assert_eq!(success.events.into_iter().next().unwrap().0, "event");
        }
        RuntimeOutput::Work(Err(error)) => {
            let _ = error.error.0;
            panic!("expected success")
        }
        RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
    }
}
