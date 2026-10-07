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

//! Typed results of repaired reads, kept for as long as the transport keeps
//! the document they were decoded from.
//!
//! A caching transport answers a read it can revalidate with the document
//! it already holds, the same `Arc`. A result is keyed by that `Arc`, the
//! type it was decoded into, and the rule set's generation, so a read that
//! gets back the same document under the same rules skips the repair and
//! the decode. Each entry holds the document weakly: the allocation cannot
//! be reused while the entry exists, so the key cannot name another
//! document, and an entry whose document the transport dropped is swept.

use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use crate::Raw;

struct Entry {
    document: Weak<Raw>,
    generation: u64,
    typed: Arc<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub struct Memo(Mutex<HashMap<(usize, TypeId), Entry>>);

fn key<T: 'static>(document: &Arc<Raw>) -> (usize, TypeId) {
    (Arc::as_ptr(document) as usize, TypeId::of::<T>())
}

impl Memo {
    /// The result decoded from `document` into `T` under `generation`.
    pub fn get<T: Send + Sync + 'static>(
        &self,
        document: &Arc<Raw>,
        generation: u64,
    ) -> Option<Arc<T>> {
        let typed = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key::<T>(document))
            .filter(|entry| entry.generation == generation)
            .map(|entry| Arc::clone(&entry.typed))?;
        typed.downcast::<T>().ok()
    }

    /// Keeps `typed`, decoded from `document` under `generation`, and drops
    /// the entries whose document is gone.
    pub fn put<T: Send + Sync + 'static>(
        &self,
        document: &Arc<Raw>,
        generation: u64,
        typed: Arc<T>,
    ) {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|_, entry| entry.document.strong_count() > 0);
        entries.insert(
            key::<T>(document),
            Entry {
                document: Arc::downgrade(document),
                generation,
                typed,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn document() -> Arc<Raw> {
        Arc::new(
            serde_json::from_value(json!({ "Id": "1" })).expect("any object is a raw document"),
        )
    }

    fn len(memo: &Memo) -> usize {
        memo.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    #[test]
    fn a_result_is_served_for_its_document_type_and_generation() {
        let memo = Memo::default();
        let document = document();
        memo.put(&document, 7, Arc::new(1_u32));

        let hit = memo
            .get::<u32>(&document, 7)
            .expect("same document, type and rules");
        assert_eq!(*hit, 1);
        assert!(memo.get::<u32>(&document, 8).is_none(), "another rule set");
        assert!(memo.get::<u64>(&document, 7).is_none(), "another type");
        assert!(
            memo.get::<u32>(&self::document(), 7).is_none(),
            "another document"
        );
    }

    #[test]
    fn a_document_the_transport_dropped_is_forgotten() {
        let memo = Memo::default();
        let dropped = document();
        memo.put(&dropped, 1, Arc::new(1_u32));
        drop(dropped);

        let kept = document();
        memo.put(&kept, 1, Arc::new(2_u32));
        assert_eq!(len(&memo), 1, "the dropped document's entry is swept");
    }
}
