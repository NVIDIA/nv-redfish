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

//! CAR (Clock with Adaptive Replacement) Cache Implementation
//!
//! Based on "CAR: Clock with Adaptive Replacement" by Bansal & Modha
//! USENIX Conference on File and Storage Technologies, 2004
//!
//! This implementation follows the exact pseudocode from the [paper](https://www.usenix.org/legacy/publications/library/proceedings/fast04/tech/full_papers/bansal/bansal.pdf).

use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;

/// A cache entry with reference bit for clock algorithm
#[derive(Debug)]
struct CacheEntry<K, V> {
    key: K,
    value: V,
    /// Reference bit: 0 or 1 as per pseudocode
    ref_bit: bool,
}

impl<K, V> CacheEntry<K, V> {
    const fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            ref_bit: false, // Always start with ref_bit = 0
        }
    }
}

/// Node in the ghost list doubly-linked structure
#[derive(Debug, Clone)]
struct GhostNode<K> {
    key: K,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Intrusive doubly linked list for ghost entries (B1, B2)
#[derive(Debug)]
struct GhostList<K> {
    entries: Vec<Option<GhostNode<K>>>,
    head: Option<usize>, // LRU end
    tail: Option<usize>, // MRU end
    free_slots: Vec<usize>,
    size: usize,
}

#[allow(clippy::manual_let_else)]
impl<K: Clone> GhostList<K> {
    fn new(capacity: usize) -> Self {
        Self {
            entries: vec![None; capacity],
            head: None,
            tail: None,
            free_slots: (0..capacity).rev().collect(),
            size: 0,
        }
    }

    /// Insert at tail (MRU position) - O(1)
    /// Returns `(slot, evicted_key)` where `evicted_key` is Some if an item was evicted
    fn insert_at_tail(&mut self, key: K) -> Option<(usize, Option<K>)> {
        // If we're at capacity, remove LRU first
        let evicted_key = if self.free_slots.is_empty() {
            self.remove_lru()
        } else {
            None
        };

        let slot = self.free_slots.pop()?;
        let new_node = GhostNode {
            key,
            prev: self.tail,
            next: None,
        };

        if let Some(old_tail) = self.tail {
            if let Some(ref mut old_tail_node) = self.entries[old_tail] {
                old_tail_node.next = Some(slot);
            }
        } else {
            self.head = Some(slot);
        }

        self.tail = Some(slot);
        self.entries[slot] = Some(new_node);
        self.size += 1;

        Some((slot, evicted_key))
    }

    /// Remove LRU (head) entry - O(1)
    fn remove_lru(&mut self) -> Option<K> {
        let head_slot = self.head?;
        let head_node = self.entries[head_slot].take()?;

        self.free_slots.push(head_slot);
        self.size -= 1;

        if self.size == 0 {
            self.head = None;
            self.tail = None;
        } else {
            self.head = head_node.next;
            if let Some(new_head) = self.head {
                if let Some(ref mut new_head_node) = self.entries[new_head] {
                    new_head_node.prev = None;
                }
            }
        }

        Some(head_node.key)
    }

    /// Remove specific slot - O(1)
    fn remove(&mut self, slot: usize) -> bool {
        let node = match self.entries[slot].take() {
            Some(node) => node,
            None => return false,
        };

        self.free_slots.push(slot);
        self.size -= 1;

        if self.size == 0 {
            self.head = None;
            self.tail = None;
        } else {
            if let Some(prev_slot) = node.prev {
                if let Some(ref mut prev_node) = self.entries[prev_slot] {
                    prev_node.next = node.next;
                }
            } else {
                self.head = node.next;
            }

            if let Some(next_slot) = node.next {
                if let Some(ref mut next_node) = self.entries[next_slot] {
                    next_node.prev = node.prev;
                }
            } else {
                self.tail = node.prev;
            }
        }

        true
    }

    const fn len(&self) -> usize {
        self.size
    }
}

/// Clock-based list for T1 and T2
#[derive(Debug)]
struct ClockList<K, V> {
    entries: Vec<Option<CacheEntry<K, V>>>,
    hand: usize, // Clock hand position
    free_slots: Vec<usize>,
    size: usize,
}

impl<K: Clone, V> ClockList<K, V> {
    fn new(capacity: usize) -> Self {
        let mut entries = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            entries.push(None);
        }
        Self {
            entries,
            hand: 0,
            free_slots: (0..capacity).rev().collect(),
            size: 0,
        }
    }

    /// Insert at tail (any available slot)
    fn insert_at_tail(&mut self, key: K, value: V) -> Option<usize> {
        let slot = self.free_slots.pop()?;
        self.entries[slot] = Some(CacheEntry::new(key, value));
        self.size += 1;
        Some(slot)
    }

    /// Get head page for clock algorithm
    fn get_head_page(&mut self) -> Option<&mut CacheEntry<K, V>> {
        // Find the entry at the current hand position
        let start_hand = self.hand;
        loop {
            if self.size == 0 {
                return None;
            }

            if self.entries[self.hand].is_some() {
                return self.entries[self.hand].as_mut();
            }

            self.advance_hand();

            // Prevent infinite loop
            if self.hand == start_hand {
                return None;
            }
        }
    }

    /// Remove head page (at current hand position)
    fn remove_head_page(&mut self) -> Option<CacheEntry<K, V>> {
        let entry = self.entries[self.hand].take()?;
        self.free_slots.push(self.hand);
        self.size -= 1;
        self.advance_hand();
        Some(entry)
    }

    const fn advance_hand(&mut self) {
        self.hand = (self.hand + 1) % self.entries.len();
    }

    fn get_mut(&mut self, slot: usize) -> Option<&mut CacheEntry<K, V>> {
        self.entries.get_mut(slot)?.as_mut()
    }

    const fn len(&self) -> usize {
        self.size
    }
}

/// Location of a key in the cache system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Location {
    T1(usize),
    T2(usize),
    B1(usize),
    B2(usize),
}

/// CAR Cache implementation following the exact pseudocode
pub struct CarCache<K, V> {
    /// Cache capacity
    c: usize,
    /// Target size for T1 (adaptive parameter)
    p: usize,

    /// T1: Recent pages (short-term utility)
    t1: ClockList<K, V>,
    /// T2: Frequent pages (long-term utility)
    t2: ClockList<K, V>,
    /// B1: Ghost list for pages evicted from T1
    b1: GhostList<K>,
    /// B2: Ghost list for pages evicted from T2
    b2: GhostList<K>,

    /// Index to track key locations
    index: HashMap<K, Location>,
}

impl<K, V> CarCache<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Create new CAR cache with given capacity
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            c: capacity,
            p: 0,
            t1: ClockList::new(capacity),
            t2: ClockList::new(capacity),
            b1: GhostList::new(capacity),
            b2: GhostList::new(capacity),
            index: HashMap::new(),
        }
    }

    /// Get value from cache
    /// Returns Some(value) if found, None if not in cache
    pub fn get(&mut self, key: &K) -> Option<&V> {
        match self.index.get(key) {
            Some(Location::T1(slot)) => {
                // Line 1-2: if (x is in T1 ∪ T2) then Set the page reference bit for x to one
                if let Some(entry) = self.t1.get_mut(*slot) {
                    entry.ref_bit = true; // Line 2: Set the page reference bit for x to one
                    Some(&entry.value)
                } else {
                    None
                }
            }
            Some(Location::T2(slot)) => {
                // Line 1-2: if (x is in T1 ∪ T2) then Set the page reference bit for x to one
                if let Some(entry) = self.t2.get_mut(*slot) {
                    entry.ref_bit = true; // Line 2: Set the page reference bit for x to one
                    Some(&entry.value)
                } else {
                    None
                }
            }
            _ => None, // Line 3: else /* cache miss */
        }
    }

    /// Insert/update value in cache following the exact pseudocode
    /// Returns `Option<V>` which denotes removed element from cache, if any
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        // Check if it's a cache hit first
        if let Some(location) = self.index.get(&key).copied() {
            match location {
                Location::T1(slot) | Location::T2(slot) => {
                    // Cache hit - update value and set reference bit
                    let entry = if matches!(location, Location::T1(_)) {
                        self.t1.get_mut(slot)
                    } else {
                        self.t2.get_mut(slot)
                    };

                    if let Some(entry) = entry {
                        entry.value = value;
                    }

                    // We are not removed anything, as we just updated value
                    return None;
                }
                _ => {
                    // Will handle B1/B2 hits below
                }
            }
        }

        let mut replaced_key = None;
        // Line 3: else /* cache miss */
        // Line 4: if (|T1| + |T2| = c) then
        if self.t1.len() + self.t2.len() == self.c {
            // Line 5: replace()
            replaced_key = self.replace();

            // Line 6: if ((x is not in B1 ∪ B2) and (|T1| + |B1| = c)) then
            if !self.is_in_b1_or_b2(&key) && (self.t1.len() + self.b1.len() == self.c) {
                // Line 7: Discard the LRU page in B1
                if let Some(discarded_key) = self.b1.remove_lru() {
                    self.index.remove(&discarded_key);
                }
            }
            // Line 8: elseif ((|T1| + |T2| + |B1| + |B2| = 2c) and (x is not in B1 ∪ B2)) then
            else if !self.is_in_b1_or_b2(&key)
                && (self.t1.len() + self.t2.len() + self.b1.len() + self.b2.len() == 2 * self.c)
            {
                // Line 9: Discard the LRU page in B2
                if let Some(discarded_key) = self.b2.remove_lru() {
                    self.index.remove(&discarded_key);
                }
            }
        }

        match self.index.get(&key).copied() {
            Some(Location::B1(slot)) => {
                // Line 14: elseif (x is in B1) then
                // Line 15: Adapt: Increase the target size for the list T1 as: p = min {p + max{1, |B2|/|B1|}, c}
                let delta = if self.b1.len() > 0 {
                    1.max(self.b2.len() / self.b1.len())
                } else {
                    1
                };
                self.p = (self.p + delta).min(self.c);

                // Remove from B1
                self.b1.remove(slot);

                // Line 16: Move x at the tail of T2. Set the page reference bit of x to 0.
                if let Some(t2_slot) = self.t2.insert_at_tail(key.clone(), value) {
                    self.index.insert(key, Location::T2(t2_slot));
                    // ref_bit is already 0 from CacheEntry::new()
                }
            }
            Some(Location::B2(slot)) => {
                // Line 17: else /* x must be in B2 */
                // Line 18: Adapt: Decrease the target size for the list T1 as: p = max {p − max{1, |B1|/|B2|}, 0}
                let delta = if self.b2.len() > 0 {
                    1.max(self.b1.len() / self.b2.len())
                } else {
                    1
                };
                self.p = self.p.saturating_sub(delta);

                // Remove from B2
                self.b2.remove(slot);

                // Line 19: Move x at the tail of T2. Set the page reference bit of x to 0.
                if let Some(t2_slot) = self.t2.insert_at_tail(key.clone(), value) {
                    self.index.insert(key, Location::T2(t2_slot));
                }
            }
            None => {
                // Line 12: if (x is not in B1 ∪ B2) then
                // Line 13: Insert x at the tail of T1. Set the page reference bit of x to 0.
                if let Some(t1_slot) = self.t1.insert_at_tail(key.clone(), value) {
                    self.index.insert(key, Location::T1(t1_slot));
                }
            }
            _ => {
                // Should not happen - T1/T2 cases handled above
            }
        }
        replaced_key
    }

    /// Line 5: `replace()` - exact implementation of pseudocode
    fn replace(&mut self) -> Option<V> {
        // Line 23: repeat
        loop {
            // Line 24: if (|T1| >= max(1, p)) then
            if self.t1.len() >= 1.max(self.p) {
                if let Some(found) = self.try_replace_from_t1() {
                    return Some(found);
                }
                self.t1.advance_hand();
            } else {
                // Line 31: else
                if let Some(found) = self.try_replace_from_t2() {
                    return Some(found);
                }
                self.t2.advance_hand();
            }
        }
        // Line 39: until (found)
    }

    /// Try to replace from T1, returns true if replacement was successful
    #[allow(clippy::if_not_else)]
    fn try_replace_from_t1(&mut self) -> Option<V> {
        if let Some(head_entry) = self.t1.get_head_page() {
            // Line 25: if (the page reference bit of head page in T1 is 0) then
            // ref_bit == false
            if !head_entry.ref_bit {
                // Line 26: found = 1;
                // Line 27: Demote the head page in T1 and make it the MRU page in B1
                if let Some(entry) = self.t1.remove_head_page() {
                    if let Some((b1_slot, evicted_key)) = self.b1.insert_at_tail(entry.key.clone())
                    {
                        // Clean up evicted key from index if any
                        if let Some(evicted) = evicted_key {
                            self.index.remove(&evicted);
                        }
                        self.index.insert(entry.key, Location::B1(b1_slot));
                    } else {
                        self.index.remove(&entry.key);
                    }
                    return Some(entry.value);
                }
            } else {
                // Line 28-29: else Set the page reference bit of head page in T1 to 0, and make it the tail page in T2
                head_entry.ref_bit = false; // Line 29: Set the page reference bit of head page in T1 to 0
                if let Some(entry) = self.t1.remove_head_page() {
                    if let Some(t2_slot) = self.t2.insert_at_tail(entry.key.clone(), entry.value) {
                        self.index.insert(entry.key, Location::T2(t2_slot));
                    }
                }
            }
        }
        None
    }

    /// Try to replace from T2, returns true if replacement was successful
    #[allow(clippy::if_not_else)]
    fn try_replace_from_t2(&mut self) -> Option<V> {
        if let Some(head_entry) = self.t2.get_head_page() {
            // Line 32: if (the page reference bit of head page in T2 is 0), then
            // ref_bit == false
            if !head_entry.ref_bit {
                // Line 33: found = 1;
                // Line 34: Demote the head page in T2 and make it the MRU page in B2
                if let Some(entry) = self.t2.remove_head_page() {
                    if let Some((b2_slot, evicted_key)) = self.b2.insert_at_tail(entry.key.clone())
                    {
                        // Clean up evicted key from index if any
                        if let Some(evicted) = evicted_key {
                            self.index.remove(&evicted);
                        }
                        self.index.insert(entry.key, Location::B2(b2_slot));
                    } else {
                        self.index.remove(&entry.key);
                    }
                    return Some(entry.value);
                }
            } else {
                // Line 35-36: else Set the page reference bit of head page in T2 to 0, and make it the tail page in T2
                head_entry.ref_bit = false; // Line 36: Set the page reference bit of head page in T2 to 0
                if let Some(entry) = self.t2.remove_head_page() {
                    if let Some(t2_slot) = self.t2.insert_at_tail(entry.key.clone(), entry.value) {
                        self.index.insert(entry.key, Location::T2(t2_slot));
                    }
                }
            }
        }
        None
    }

    /// Helper function to check if key is in B1 or B2
    fn is_in_b1_or_b2(&self, key: &K) -> bool {
        matches!(self.index.get(key), Some(Location::B1(_) | Location::B2(_)))
    }

    /// Get current cache size (items in T1 + T2)
    #[must_use]
    pub const fn len(&self) -> usize {
        self.t1.len() + self.t2.len()
    }

    /// Check if cache is empty
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache capacity
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.c
    }

    /// Get current adaptation parameter
    #[must_use]
    pub const fn adaptation_parameter(&self) -> usize {
        self.p
    }
}

pub(crate) type TypeErasedCarCache<K> = CarCache<K, Box<dyn Any + Send + Sync>>;

impl<K> TypeErasedCarCache<K>
where
    K: Eq + Hash + Clone,
{
    pub(crate) fn get_typed<T: 'static + Send + Sync>(&mut self, key: &K) -> Option<&T> {
        self.get(key)?.downcast_ref::<T>()
    }

    pub(crate) fn put_typed<T: 'static + Send + Sync>(&mut self, key: K, value: T) -> Option<T> {
        let ret = self.put(key, Box::new(value) as Box<dyn Any + Send + Sync>);
        ret?.downcast::<T>().ok().map(|r| *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_cache_with_invariant_check<K, V>(
        cache: &mut CarCache<K, V>,
        items: impl Iterator<Item = (K, V)>,
    ) where
        K: Eq + std::hash::Hash + Clone,
    {
        for (key, value) in items {
            cache.put(key, value);
            assert_car_invariants(cache);
        }
    }

    fn access_items_with_invariant_check<K, V>(
        cache: &mut CarCache<K, V>,
        keys: impl Iterator<Item = K>,
    ) where
        K: Eq + std::hash::Hash + Clone,
    {
        for key in keys {
            cache.get(&key);
            assert_car_invariants(cache);
        }
    }

    fn assert_car_invariants<K, V>(cache: &CarCache<K, V>)
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let c = cache.capacity();
        let t1_size = cache.t1.len();
        let t2_size = cache.t2.len();
        let b1_size = cache.b1.len();
        let b2_size = cache.b2.len();
        let p = cache.adaptation_parameter();

        let state_info = format!(
            "Cache state: T1={}, T2={}, B1={}, B2={}, c={}, p={}",
            t1_size, t2_size, b1_size, b2_size, c, p
        );

        // I1: 0 ≤ |T1| + |T2| ≤ c
        assert!(
            t1_size + t2_size <= c,
            "I1 violated: |T1| + |T2| = {} > c = {}. {}",
            t1_size + t2_size,
            c,
            state_info
        );

        // I2: 0 ≤ |T1| + |B1| ≤ c
        assert!(
            t1_size + b1_size <= c,
            "I2 violated: |T1| + |B1| = {} > c = {}. {}",
            t1_size + b1_size,
            c,
            state_info
        );

        // I3: 0 ≤ |T2| + |B2| ≤ 2c
        assert!(
            t2_size + b2_size <= 2 * c,
            "I3 violated: |T2| + |B2| = {} > 2c = {}. {}",
            t2_size + b2_size,
            2 * c,
            state_info
        );

        // I4: 0 ≤ |T1| + |T2| + |B1| + |B2| ≤ 2c
        assert!(
            t1_size + t2_size + b1_size + b2_size <= 2 * c,
            "I4 violated: |T1| + |T2| + |B1| + |B2| = {} > 2c = {}. {}",
            t1_size + t2_size + b1_size + b2_size,
            2 * c,
            state_info
        );

        // I5: If |T1| + |T2| < c, then B1 ∪ B2 is empty
        if t1_size + t2_size < c {
            assert!(
                b1_size == 0 && b2_size == 0,
                "I5 violated: |T1| + |T2| = {} < c = {} but B1 or B2 not empty. {}",
                t1_size + t2_size,
                c,
                state_info
            );
        }

        // I6: If |T1| + |B1| + |T2| + |B2| ≥ c, then |T1| + |T2| = c
        if t1_size + b1_size + t2_size + b2_size >= c {
            assert!(
                t1_size + t2_size == c,
                "I6 violated: total directory size {} ≥ c = {} but |T1| + |T2| = {} ≠ c. {}",
                t1_size + b1_size + t2_size + b2_size,
                c,
                t1_size + t2_size,
                state_info
            );
        }

        // I7: Once cache is full, it remains full
        if t1_size + t2_size == c {
            assert_eq!(
                cache.len(),
                c,
                "I7: Cache should remain at capacity once full. {}",
                state_info
            );
        }

        assert!(
            p <= c,
            "Adaptation parameter p={} should not exceed capacity c={}. {}",
            p,
            c,
            state_info
        );
        assert_eq!(
            cache.len(),
            t1_size + t2_size,
            "Cache length mismatch. {}",
            state_info
        );
    }

    fn create_eviction_pressure(cache: &mut CarCache<String, i32>, rounds: i32) {
        for round in 0..rounds {
            cache.put(format!("b1_source_{}", round), round + 100);
            assert_car_invariants(cache);

            cache.put(format!("b2_source_{}", round), round + 200);
            cache.get(&format!("b2_source_{}", round));
            assert_car_invariants(cache);

            cache.put(format!("pressure_{}", round), round + 300);
            assert_car_invariants(cache);
        }
    }

    fn promote_all_to_t2(cache: &mut CarCache<i32, i32>, range: std::ops::Range<i32>) {
        for i in range.clone() {
            cache.put(i, i);
            cache.get(&i);
            assert_car_invariants(cache);
        }
    }

    fn create_t1_t2_mix(cache: &mut CarCache<String, i32>, prefix: &str, count: i32) {
        fill_cache_with_invariant_check(
            cache,
            (0..count).map(|i| (format!("{}_{}", prefix, i), i)),
        );
        access_items_with_invariant_check(
            cache,
            (0..count / 2).map(|i| format!("{}_{}", prefix, i)),
        );
    }

    fn verify_directory_state<K, V>(cache: &CarCache<K, V>) -> (usize, usize, usize, usize, usize)
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let t1_size = cache.t1.len();
        let t2_size = cache.t2.len();
        let b1_size = cache.b1.len();
        let b2_size = cache.b2.len();
        let total = t1_size + t2_size + b1_size + b2_size;

        (t1_size, t2_size, b1_size, b2_size, total)
    }

    fn create_ghost_hits(
        cache: &mut CarCache<String, i32>,
        prefix: &str,
        range: std::ops::Range<i32>,
        value_offset: i32,
    ) {
        for i in range {
            cache.put(format!("{}_{}", prefix, i), i + value_offset);
            assert_car_invariants(cache);
        }
    }

    #[test]
    fn test_ghost_list_basic_operations() {
        let mut ghost_list = GhostList::new(3);

        assert_eq!(ghost_list.len(), 0);
        assert_eq!(ghost_list.remove_lru(), None);

        let (_slot1, _) = ghost_list.insert_at_tail("a").unwrap();
        assert_eq!(ghost_list.len(), 1);

        let (slot2, _) = ghost_list.insert_at_tail("b").unwrap();
        assert_eq!(ghost_list.len(), 2);

        assert_eq!(ghost_list.remove_lru(), Some("a"));
        assert_eq!(ghost_list.len(), 1);

        assert!(ghost_list.remove(slot2));
        assert_eq!(ghost_list.len(), 0);
    }

    #[test]
    fn test_clock_list_basic_operations() {
        let mut clock_list = ClockList::new(3);

        assert_eq!(clock_list.len(), 0);
        assert!(clock_list.get_head_page().is_none());

        let slot1 = clock_list.insert_at_tail("a", 1).unwrap();
        assert_eq!(clock_list.len(), 1);

        let slot2 = clock_list.insert_at_tail("b", 2).unwrap();
        assert_eq!(clock_list.len(), 2);

        assert_eq!(clock_list.get_mut(slot1).unwrap().value, 1);
        assert_eq!(clock_list.get_mut(slot2).unwrap().value, 2);

        let entry = clock_list.get_mut(slot1).unwrap();
        assert_eq!(entry.ref_bit, false);
    }

    #[test]
    fn test_adaptation_parameter_increase_on_b1_hit() {
        let mut cache = CarCache::new(4);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        let initial_p = cache.adaptation_parameter();
        cache.get(&"a");

        cache.put("e", 5);
        cache.put("f", 6);

        cache.put("c", 10);

        assert!(cache.adaptation_parameter() > initial_p);
        assert!(cache.adaptation_parameter() <= cache.capacity());
    }

    #[test]
    fn test_adaptation_parameter_decrease_on_b2_hit() {
        let mut cache = CarCache::new(4);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        cache.get(&"a");

        cache.put("e", 5);
        cache.put("f", 6);

        cache.put("c", 10);

        let p_before = cache.adaptation_parameter();

        cache.put("f", 6);
        cache.get(&"f");
        cache.put("g", 7);
        cache.get(&"g");
        cache.put("x", 7);
        cache.put("y", 7);
        cache.put("z", 7);

        cache.put("a", 10);

        assert!(cache.adaptation_parameter() < p_before);
    }

    #[test]
    fn test_clock_algorithm_reference_bit_behavior() {
        let mut cache = CarCache::new(3);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        cache.get(&"a");

        cache.put("d", 4);
        cache.put("e", 5);

        assert!(cache.get(&"a").is_some());
        assert!(cache.len() <= 3);
    }

    #[test]
    fn test_ghost_list_lru_behavior() {
        let mut ghost_list = GhostList::new(3);

        let _ = ghost_list.insert_at_tail("first");
        let _ = ghost_list.insert_at_tail("second");
        let _ = ghost_list.insert_at_tail("third");

        assert_eq!(ghost_list.remove_lru(), Some("first"));
        assert_eq!(ghost_list.remove_lru(), Some("second"));
        assert_eq!(ghost_list.remove_lru(), Some("third"));
        assert_eq!(ghost_list.remove_lru(), None);
    }

    #[test]
    fn test_directory_replacement_constraints() {
        let mut cache = CarCache::new(3);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.get(&"a");
        cache.put("c", 3);
        cache.get(&"c");
        cache.put("d", 4);
        cache.put("e", 5);

        assert_eq!(cache.t1.len(), 1);
        assert_eq!(cache.t2.len(), 2);
    }

    #[test]
    fn test_large_cache_reference_bit_behavior() {
        let mut cache = CarCache::new(1000);

        for i in 0..800 {
            cache.put(format!("frequent_{}", i), i);
            cache.get(&format!("frequent_{}", i)); // Set reference bit
        }

        for i in 0..200 {
            cache.put(format!("rare_{}", i), i);
        }

        for i in 0..400 {
            cache.put(format!("new_{}", i), i);
        }

        let frequent_survivors = (0..800)
            .filter(|&i| cache.get(&format!("frequent_{}", i)).is_some())
            .count();

        let rare_survivors = (0..200)
            .filter(|&i| cache.get(&format!("rare_{}", i)).is_some())
            .count();

        assert!(frequent_survivors as f64 / 800.0 >= rare_survivors as f64 / 200.0);
    }

    #[test]
    fn test_large_cache_scan_resistance() {
        let mut cache = CarCache::new(1000);

        let working_set: Vec<String> = (0..200).map(|i| format!("working_{}", i)).collect();
        for key in &working_set {
            cache.put(key.clone(), 1);
            cache.get(key);
        }

        for i in 0..800 {
            cache.put(format!("filler_{}", i), i);
        }

        for i in 0..500 {
            cache.put(format!("scan_{}", i), i);
        }

        let survivors = working_set
            .iter()
            .filter(|key| cache.get(key).is_some())
            .count();

        assert_eq!(survivors, 200);
        assert_eq!(cache.len(), cache.capacity());
        assert!(cache.adaptation_parameter() <= cache.capacity());
    }

    #[test]
    fn test_cache_adaptation_bounds() {
        let mut cache = CarCache::new(10);
        let mut p_values = Vec::new();

        let working_set = (0..15).map(|i| format!("item_{}", i)).collect::<Vec<_>>();

        for i in 0..8 {
            cache.put(working_set[i].clone(), i);
        }

        for i in 0..4 {
            cache.get(&working_set[i]);
        }

        p_values.push(cache.adaptation_parameter());
        for cycle in 0..3 {
            for (round, item) in working_set.iter().enumerate() {
                cache.put(item.clone(), cycle * 100 + round);

                let p_after = cache.adaptation_parameter();
                p_values.push(p_after);

                assert!(
                    p_after <= cache.capacity(),
                    "Adaptation parameter {} exceeds capacity {} at cycle {} round {}",
                    p_after,
                    cache.capacity(),
                    cycle,
                    round
                );

                if round % 3 == 0 && round > 0 {
                    cache.get(&working_set[round - 1]);
                }
            }
        }

        for (i, &p) in p_values.iter().enumerate() {
            assert!(
                p <= cache.capacity(),
                "p={} > c={} at step {}",
                p,
                cache.capacity(),
                i
            );
        }

        let p_changed = p_values.iter().any(|&p| p != p_values[0]);
        assert!(
            p_changed,
            "NOTE: Adaptation parameter remained at {} (may need different workload)",
            p_values[0]
        );
        assert_eq!(cache.adaptation_parameter(), 5);
    }

    #[test]
    fn test_put_return_values_eviction() {
        let mut cache = CarCache::new(3);

        assert_eq!(cache.put("a", 1), None);
        assert_eq!(cache.put("b", 2), None);
        assert_eq!(cache.put("c", 3), None);

        assert_eq!(cache.put("d", 4), Some(1));
        assert_eq!(cache.put("e", 5), Some(2));

        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"d"), Some(&4));
        assert_eq!(cache.get(&"e"), Some(&5));
    }

    #[test]
    fn test_put_return_values_t1_t2_eviction() {
        let mut cache = CarCache::new(4);

        assert_eq!(cache.put("t1_a", 1), None);
        assert_eq!(cache.put("t1_b", 2), None);

        cache.get(&"t1_a");
        cache.get(&"t1_b");

        assert_eq!(cache.put("t1_c", 3), None);
        assert_eq!(cache.put("t1_d", 4), None);

        assert_eq!(cache.put("new1", 10), Some(3));
    }

    #[test]
    fn test_car_invariants_i3_stress() {
        let mut cache = CarCache::new(5);

        promote_all_to_t2(&mut cache, 0..5);

        for i in 5..20 {
            cache.put(i, i);
            assert_car_invariants(&cache);
            cache.get(&i);
            assert_car_invariants(&cache);
        }

        fill_cache_with_invariant_check(&mut cache, (0..5).map(|i| (i, i + 100)));

        let (_, t2_size, _, b2_size, _) = verify_directory_state(&cache);
        assert!(
            t2_size + b2_size > 0,
            "Should have some T2/B2 entries to test I3"
        );
    }

    #[test]
    fn test_car_invariants_i4_maximum_directory() {
        let mut cache = CarCache::new(8);

        create_t1_t2_mix(&mut cache, "t1", 8);
        create_eviction_pressure(&mut cache, 10);
        create_ghost_hits(&mut cache, "t1", 0..4, 1000);

        let (_, _, _, _, total) = verify_directory_state(&cache);
        let max_allowed = 2 * cache.capacity();

        assert!(
            total >= cache.capacity(),
            "Directory should be substantial for meaningful I4 test"
        );
        assert!(
            total <= max_allowed,
            "I4: Directory size {} should not exceed 2c={}",
            total,
            max_allowed
        );
    }

    #[test]
    fn test_car_invariant_i6_directory_full_cache_full() {
        let mut cache = CarCache::new(6);

        create_t1_t2_mix(&mut cache, "initial", 6);

        for i in 6..15 {
            cache.put(format!("evict_{}", i), i);
            assert_car_invariants(&cache);

            if i % 2 == 0 {
                cache.get(&format!("evict_{}", i));
                assert_car_invariants(&cache);
            }
        }

        create_ghost_hits(&mut cache, "initial", 0..3, 1000);

        let (t1_size, t2_size, _b1_size, _b2_size, total_dir) = verify_directory_state(&cache);

        if total_dir >= cache.capacity() {
            assert_eq!(
                t1_size + t2_size,
                cache.capacity(),
                "I6: When directory size {} ≥ c={}, cache should be full but |T1|+|T2|={}",
                total_dir,
                cache.capacity(),
                t1_size + t2_size
            );
        } else {
            panic!(
                "Test setup failed: Directory size {} should be ≥ c={}",
                total_dir,
                cache.capacity()
            );
        }
    }

    #[test]
    fn test_car_invariant_i7_cache_remains_full() {
        let mut cache = CarCache::new(8);

        for i in 0..8 {
            cache.put(format!("fill_{}", i), i);
            assert_car_invariants(&cache);
        }

        assert_eq!(cache.len(), cache.capacity(), "Cache should be at capacity");

        for round in 0..20 {
            cache.put(format!("new_{}", round), round + 100);
            assert_car_invariants(&cache);
            assert_eq!(
                cache.len(),
                cache.capacity(),
                "I7: Cache should remain full after adding new item in round {}",
                round
            );

            cache.get(&format!("new_{}", round));
            assert_car_invariants(&cache);
            assert_eq!(
                cache.len(),
                cache.capacity(),
                "I7: Cache should remain full after accessing item in round {}",
                round
            );

            cache.put(format!("new_{}", round), round + 200);
            assert_car_invariants(&cache);
            assert_eq!(
                cache.len(),
                cache.capacity(),
                "I7: Cache should remain full after updating item in round {}",
                round
            );

            if round > 5 {
                cache.put(format!("fill_{}", round % 8), round + 300);
                assert_car_invariants(&cache);
                assert_eq!(
                    cache.len(),
                    cache.capacity(),
                    "I7: Cache should remain full after B1/B2 hit in round {}",
                    round
                );
            }
        }
    }
}
