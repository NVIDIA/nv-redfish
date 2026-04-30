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

#[derive(Default)]
pub(crate) struct FlatRoundRobin {
    order: Vec<GeneratorId>,
    cursor: usize,
}

impl FlatRoundRobin {
    pub(crate) fn insert(&mut self, generator_id: GeneratorId) {
        self.order.push(generator_id);
    }

    pub(crate) fn remove(&mut self, generator_id: GeneratorId) -> bool {
        self.order
            .iter()
            .position(|candidate| *candidate == generator_id)
            .map(|index| {
                self.order.remove(index);
                if self.order.is_empty() {
                    self.cursor = 0;
                } else if index < self.cursor {
                    self.cursor -= 1;
                } else if self.cursor >= self.order.len() {
                    self.cursor = 0;
                }
            })
            .is_some()
    }

    pub(crate) fn find_map<T>(&mut self, mut f: impl FnMut(GeneratorId) -> Option<T>) -> Option<T> {
        let scan_len = self.order.len();
        (0..scan_len).find_map(|_| self.next_candidate().and_then(&mut f))
    }

    fn next_candidate(&mut self) -> Option<GeneratorId> {
        (!self.order.is_empty()).then(|| {
            let index = self.cursor;
            self.cursor = (self.cursor + 1) % self.order.len();
            self.order[index]
        })
    }
}
