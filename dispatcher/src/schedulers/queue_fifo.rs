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

//! FIFO queue discipline.

use std::collections::VecDeque;

use super::bounded_queue::{BoundedQueueBuilder, QueueDiscipline, QueueEntryId};

/// First-in, first-out queue discipline.
#[derive(Debug, Default)]
pub struct Fifo {
    entries: VecDeque<QueueEntryId>,
}

impl Fifo {
    /// Empty FIFO discipline.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}

impl<M> QueueDiscipline<M> for Fifo {
    fn push(&mut self, id: QueueEntryId, _meta: &M) {
        self.entries.push_back(id);
    }

    fn take_next(&mut self) -> Option<QueueEntryId> {
        self.entries.pop_front()
    }

    fn remove(&mut self, id: QueueEntryId) -> bool {
        let Some(position) = self.entries.iter().position(|&entry| entry == id) else {
            return false;
        };
        let _ = self.entries.remove(position);
        true
    }
}

impl<P, D> BoundedQueueBuilder<P, D> {
    /// Select the built-in FIFO discipline.
    #[must_use]
    pub fn fifo(self) -> BoundedQueueBuilder<P, Fifo> {
        self.discipline(Fifo::new())
    }
}
