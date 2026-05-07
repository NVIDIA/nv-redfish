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

//! Scheduler abstractions.
//!
//! The scheduler operates only on abstract metadata — identity, readiness, next
//! ready time, cost, in-flight state, and capacity/budget state. It is not
//! parameterized by any concrete request type; concrete work is produced below
//! the scheduler by [`crate::Generator::take_next`] for the selected generator.

use crate::generator::CostUnits;

/// A request produced by a scheduling item, shaped for scheduler bookkeeping.
///
/// Phase 0 only requires the cost projection. Later phases may extend this
/// trait with class identity or in-flight-budget hints, but must not break the
/// invariant that schedulers operate without knowledge of concrete request
/// payloads.
pub trait ScheduledRequest {
    /// Cost of the request, used for admission and fairness accounting.
    fn cost(&self) -> CostUnits;
}
