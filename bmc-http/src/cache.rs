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
//! This implementation follows the pseudocode from the [paper](https://www.usenix.org/legacy/publications/library/proceedings/fast04/tech/full_papers/bansal/bansal.pdf),
//! with one clock-native reading: "make it the tail page in T2"
//! (line 36) is realized by clearing the reference bit and advancing
//! the hand — in a circular list that leaves the page as the last one
//! the hand revisits.

use std::any::Any;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

/// Information about an evicted cache entry.
///
/// When an entry is evicted from the cache, this struct holds both the key
/// and value of the evicted entry. This is particularly useful for cleaning
/// up related resources (like ETags) when entries are evicted.
#[derive(Debug)]
pub struct Evicted<K, V> {
    /// The key of the evicted entry
    pub key: K,
    /// The value of the evicted entry
    pub value: V,
}

impl<K, V> Evicted<K, V> {
    /// Create a new Evicted struct
    const fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
}

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
#[derive(Debug)]
struct GhostNode<K> {
    key: K,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Intrusive doubly linked list for ghost entries (B1, B2)
#[derive(Debug)]
struct GhostList<K> {
    /// Grows on demand up to `capacity`, so an idle cache holds no
    /// slot storage.
    entries: Vec<Option<GhostNode<K>>>,
    capacity: usize,
    head: Option<usize>, // LRU end
    tail: Option<usize>, // MRU end
    free_slots: Vec<usize>,
    size: usize,
}

impl<K: Clone> GhostList<K> {
    const fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
            head: None,
            tail: None,
            free_slots: Vec::new(),
            size: 0,
        }
    }

    fn acquire_slot(&mut self) -> Option<usize> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        if self.entries.len() < self.capacity {
            self.entries.push(None);
            Some(self.entries.len() - 1)
        } else {
            None
        }
    }

    /// Insert at tail (MRU position) - O(1)
    /// Returns the slot the key was stored in. Callers must keep the
    /// list below capacity; see [`CarCache::new`] for the sizing.
    fn insert_at_tail(&mut self, key: K) -> Option<usize> {
        debug_assert!(self.size < self.capacity, "insert into a full ghost list");
        let slot = self.acquire_slot()?;
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

        Some(slot)
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
        let Some(node) = self.entries[slot].take() else {
            return false;
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

/// Node in the clock's circular intrusive list
#[derive(Debug)]
struct ClockNode<K, V> {
    entry: CacheEntry<K, V>,
    prev: usize,
    next: usize,
}

/// Clock-based list for T1 and T2.
///
/// Occupied slots form a circular doubly-linked ring in insertion
/// order; the hand points at the head (next victim candidate), so head
/// access, removal and hand advancement are O(1) regardless of how
/// sparsely the slot vector is populated.
#[derive(Debug)]
struct ClockList<K, V> {
    /// Grows on demand up to `capacity`, so an idle cache holds no
    /// slot storage.
    nodes: Vec<Option<ClockNode<K, V>>>,
    capacity: usize,
    /// Clock hand: slot of the current head, `None` when empty
    hand: Option<usize>,
    free_slots: Vec<usize>,
    size: usize,
}

impl<K: Clone, V> ClockList<K, V> {
    const fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::new(),
            capacity,
            hand: None,
            free_slots: Vec::new(),
            size: 0,
        }
    }

    fn acquire_slot(&mut self) -> Option<usize> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        if self.nodes.len() < self.capacity {
            self.nodes.push(None);
            Some(self.nodes.len() - 1)
        } else {
            None
        }
    }

    /// Insert at tail — immediately behind the hand, so the new entry
    /// is the last the clock visits — O(1). On failure (list full, or
    /// a broken hand invariant) the pair is handed back untouched.
    fn insert_at_tail(&mut self, key: K, value: V) -> Result<usize, (K, V)> {
        let links = match self.hand {
            Some(hand) => match self.nodes.get(hand).and_then(Option::as_ref) {
                Some(node) => Some((node.prev, hand)),
                None => return Err((key, value)),
            },
            None => None,
        };
        let Some(slot) = self.acquire_slot() else {
            return Err((key, value));
        };
        let (prev, next) = links.unwrap_or((slot, slot));
        self.nodes[slot] = Some(ClockNode {
            entry: CacheEntry::new(key, value),
            prev,
            next,
        });
        if links.is_some() {
            if let Some(prev_node) = self.nodes.get_mut(prev).and_then(Option::as_mut) {
                prev_node.next = slot;
            }
            if let Some(next_node) = self.nodes.get_mut(next).and_then(Option::as_mut) {
                next_node.prev = slot;
            }
        } else {
            self.hand = Some(slot);
        }
        self.size += 1;
        Ok(slot)
    }

    /// Get head page for clock algorithm - O(1)
    fn get_head_page(&mut self) -> Option<&mut CacheEntry<K, V>> {
        let hand = self.hand?;
        self.nodes
            .get_mut(hand)?
            .as_mut()
            .map(|node| &mut node.entry)
    }

    /// Remove head page (at the hand) - O(1)
    fn remove_head_page(&mut self) -> Option<CacheEntry<K, V>> {
        let hand = self.hand?;
        let node = self.nodes.get_mut(hand)?.take()?;
        self.free_slots.push(hand);
        self.size -= 1;
        if self.size == 0 {
            self.hand = None;
        } else {
            if let Some(prev_node) = self.nodes.get_mut(node.prev).and_then(Option::as_mut) {
                prev_node.next = node.next;
            }
            if let Some(next_node) = self.nodes.get_mut(node.next).and_then(Option::as_mut) {
                next_node.prev = node.prev;
            }
            self.hand = Some(node.next);
        }
        Some(node.entry)
    }

    /// Move the hand past the current head - O(1)
    fn advance_hand(&mut self) {
        if let Some(hand) = self.hand {
            if let Some(node) = self.nodes.get(hand).and_then(Option::as_ref) {
                self.hand = Some(node.next);
            }
        }
    }

    fn get_mut(&mut self, slot: usize) -> Option<&mut CacheEntry<K, V>> {
        self.nodes
            .get_mut(slot)?
            .as_mut()
            .map(|node| &mut node.entry)
    }

    fn get(&self, slot: usize) -> Option<&CacheEntry<K, V>> {
        self.nodes.get(slot)?.as_ref().map(|node| &node.entry)
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
pub struct CarCache<K, V, S = RandomState> {
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
    index: HashMap<K, Location, S>,
}

impl<K: Clone, V> CarCache<K, V> {
    /// Create new CAR cache with given capacity.
    ///
    /// A capacity of 0 creates a disabled cache that never stores entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self::with_hasher(capacity, RandomState::new())
    }
}

impl<K: Clone, V, S: BuildHasher> CarCache<K, V, S> {
    /// Create a CAR cache with a custom hash builder.
    #[must_use]
    pub const fn with_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            c: capacity,
            p: 0,
            t1: ClockList::new(capacity),
            t2: ClockList::new(capacity),
            // One slack slot: a put demotes at most one page into a
            // ghost list and ends with both lists at or below c (the
            // guarded discards, lines 6-9, or the requested ghost's
            // own removal restore the bound). Only B2 can transiently
            // reach c+1, keeping the requested key's ghost alive for
            // its adaptation hit; B1 peaks at c and is sized alike.
            b1: GhostList::new(capacity.saturating_add(1)),
            b2: GhostList::new(capacity.saturating_add(1)),
            index: HashMap::with_hasher(hasher),
        }
    }
}

impl<K, V, S: BuildHasher> CarCache<K, V, S>
where
    K: Eq + Hash + Clone,
{
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
    /// Returns `Option<Evicted<K, V>>` containing the evicted entry (key and value)
    /// if an entry was evicted from the cache, or `None` if no eviction occurred.
    pub fn put(&mut self, key: K, value: V) -> Option<Evicted<K, V>> {
        if self.c == 0 {
            return None;
        }

        // Check if it's a cache hit first
        let hit = match self.index.get(&key).copied() {
            Some(Location::T1(slot)) => self.t1.get_mut(slot),
            Some(Location::T2(slot)) => self.t2.get_mut(slot),
            // B1/B2 hits are handled in the miss path below.
            _ => None,
        };
        if let Some(entry) = hit {
            // Line 2: a hit sets the page reference bit, whether the
            // request reads or refreshes.
            entry.ref_bit = true;
            entry.value = value;
            return None;
        }

        let mut evicted = None;
        // The key's post-replace location; when the cache is not full,
        // invariant I5 guarantees B1 ∪ B2 is empty, so `None` is exact.
        let mut location = None;
        debug_assert!(
            self.t1.len() + self.t2.len() == self.c || self.b1.len() + self.b2.len() == 0,
            "I5 violated: ghosts exist while the cache is not full"
        );
        // Line 3: else /* cache miss */
        // Line 4: if (|T1| + |T2| = c) then
        if self.t1.len() + self.t2.len() == self.c {
            // Line 5: replace()
            evicted = self.replace();

            // replace() can discard ghosts, so look the key up after it;
            // one lookup serves lines 6, 8 and the B1/B2 dispatch below.
            location = self.index.get(&key).copied();
            let in_ghosts = matches!(location, Some(Location::B1(_) | Location::B2(_)));

            // Line 6: if ((x is not in B1 ∪ B2) and (|T1| + |B1| = c)) then
            if !in_ghosts && (self.t1.len() + self.b1.len() == self.c) {
                // Line 7: Discard the LRU page in B1
                if let Some(discarded_key) = self.b1.remove_lru() {
                    self.index.remove(&discarded_key);
                }
            }
            // Line 8: elseif ((|T1| + |T2| + |B1| + |B2| = 2c) and (x is not in B1 ∪ B2)) then
            else if !in_ghosts
                && (self.t1.len() + self.t2.len() + self.b1.len() + self.b2.len() == 2 * self.c)
            {
                // Line 9: Discard the LRU page in B2
                if let Some(discarded_key) = self.b2.remove_lru() {
                    self.index.remove(&discarded_key);
                }
            }
        }

        match location {
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
                self.move_to_t2(key, value);
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
                self.move_to_t2(key, value);
            }
            None => {
                // Line 12: if (x is not in B1 ∪ B2) then
                // Line 13: Insert x at the tail of T1. Set the page reference bit of x to 0.
                if let Ok(t1_slot) = self.t1.insert_at_tail(key.clone(), value) {
                    self.index.insert(key, Location::T1(t1_slot));
                }
            }
            Some(Location::T1(_) | Location::T2(_)) => {
                debug_assert!(false, "T1/T2 hits are handled before the miss path");
            }
        }
        evicted.map(|e| Evicted::new(e.key, e.value))
    }

    /// Move an already-indexed key to the tail of T2 and repoint its
    /// index entry, without cloning the key. If T2 cannot take the
    /// entry the index entry is removed, keeping index and lists
    /// consistent.
    fn move_to_t2(&mut self, key: K, value: V) {
        match self.t2.insert_at_tail(key, value) {
            Ok(t2_slot) => {
                if let Some(moved) = self.t2.get(t2_slot) {
                    if let Some(location) = self.index.get_mut(&moved.key) {
                        *location = Location::T2(t2_slot);
                    }
                }
            }
            Err((key, _value)) => {
                self.index.remove(&key);
            }
        }
    }

    /// Line 5: `replace()` - exact implementation of pseudocode
    fn replace(&mut self) -> Option<CacheEntry<K, V>> {
        // Line 23: repeat
        loop {
            // Line 24: if (|T1| >= max(1, p)) then
            if self.t1.len() >= 1.max(self.p) {
                if let Some(found) = self.try_replace_from_t1() {
                    return Some(found);
                }
                // No advance here: a T1 pass that found no victim has
                // recirculated the head to T2, which already moved the
                // hand to the next page. An extra advance would skip
                // that page without examining its reference bit.
            } else {
                // Line 31: else
                if let Some(found) = self.try_replace_from_t2() {
                    return Some(found);
                }
                // A T2 pass that found no victim only cleared the head's
                // reference bit; move the hand past it.
                self.t2.advance_hand();
            }
        }
        // Line 39: until (found)
    }

    /// Try to replace from T1, returns the evicted entry if replacement was successful
    fn try_replace_from_t1(&mut self) -> Option<CacheEntry<K, V>> {
        if let Some(head_entry) = self.t1.get_head_page() {
            // Line 25: if (the page reference bit of head page in T1 is 0) then
            // ref_bit == false
            #[allow(clippy::bool_comparison)] // Allow to match paper
            if head_entry.ref_bit == false {
                // Line 26: found = 1;
                // Line 27: Demote the head page in T1 and make it the MRU page in B1
                if let Some(entry) = self.t1.remove_head_page() {
                    if let Some(b1_slot) = self.b1.insert_at_tail(entry.key.clone()) {
                        // The key is already indexed (it was in T1):
                        // update the location in place.
                        if let Some(location) = self.index.get_mut(&entry.key) {
                            *location = Location::B1(b1_slot);
                        }
                    } else {
                        self.index.remove(&entry.key);
                    }
                    return Some(entry);
                }
            } else {
                // Line 28-29: else Set the page reference bit of head page in T1 to 0, and make it the tail page in T2
                head_entry.ref_bit = false; // Line 29: Set the page reference bit of head page in T1 to 0
                if let Some(entry) = self.t1.remove_head_page() {
                    self.move_to_t2(entry.key, entry.value);
                }
            }
        }
        None
    }

    /// Try to replace from T2, returns the evicted entry if replacement was successful
    fn try_replace_from_t2(&mut self) -> Option<CacheEntry<K, V>> {
        if let Some(head_entry) = self.t2.get_head_page() {
            // Line 32: if (the page reference bit of head page in T2 is 0), then
            // ref_bit == false
            #[allow(clippy::bool_comparison)] // Allow to match paper
            if head_entry.ref_bit == false {
                // Line 33: found = 1;
                // Line 34: Demote the head page in T2 and make it the MRU page in B2
                if let Some(entry) = self.t2.remove_head_page() {
                    if let Some(b2_slot) = self.b2.insert_at_tail(entry.key.clone()) {
                        // The key is already indexed (it was in T2):
                        // update the location in place.
                        if let Some(location) = self.index.get_mut(&entry.key) {
                            *location = Location::B2(b2_slot);
                        }
                    } else {
                        self.index.remove(&entry.key);
                    }
                    return Some(entry);
                }
            } else {
                // Line 35-36: else Set the page reference bit of head page in T2 to 0, and make it the tail page in T2
                //
                // In a clock, "make it the tail" is the hand passing
                // over the page: clear the bit and leave the entry in
                // its slot; once the caller advances the hand, this
                // page is the last the hand will revisit.
                head_entry.ref_bit = false; // Line 36: Set the page reference bit of head page in T2 to 0
            }
        }
        None
    }
}

impl<K: Clone, V, S> CarCache<K, V, S> {
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

    /// Put a typed value into the cache and return the evicted key if any.
    ///
    /// Returns `Some(key)` if an entry was evicted from the cache, `None` otherwise.
    pub(crate) fn put_typed<T: 'static + Send + Sync>(&mut self, key: K, value: T) -> Option<K> {
        let evicted = self.put(key, Box::new(value) as Box<dyn Any + Send + Sync>);
        evicted.map(|e| e.key)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TypeA {
        id: String,
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TypeB {
        id: String,
    }

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

        let _slot1 = ghost_list.insert_at_tail("a").unwrap();
        assert_eq!(ghost_list.len(), 1);

        let slot2 = ghost_list.insert_at_tail("b").unwrap();
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
    fn test_clock_examines_every_page_during_replacement() {
        // T1 = [a(ref=1), b, c]: the clock recirculates `a` to T2 and
        // must then examine `b` — the victim is `b`, not `c`.
        let mut cache = CarCache::new(3);
        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);
        cache.get(&"a");

        let evicted = cache.put("d", 4).expect("full cache evicts");
        assert_eq!(evicted.key, "b");
        assert_eq!(evicted.value, 2);
        assert_car_invariants(&cache);
        // `a` survived via recirculation to T2.
        assert_eq!(cache.get(&"a"), Some(&1));
    }

    #[test]
    fn test_put_hit_sets_reference_bit() {
        // A refresh is a request for x (paper line 2): the refreshed
        // entry must survive the next replacement via recirculation.
        let mut cache = CarCache::new(3);
        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);
        cache.put("a", 10);

        let evicted = cache.put("d", 4).expect("full cache evicts");
        assert_eq!(evicted.key, "b");
        assert_car_invariants(&cache);
        assert_eq!(cache.get(&"a"), Some(&10));
    }

    #[test]
    fn test_mid_put_demotion_spares_requested_ghost() {
        // Build |B2| = c with the requested key as B2's LRU (forcing
        // B1 empty and a full 2c directory), so replace()'s demotion
        // inside the same put pushes B2 to c+1 before the hit is
        // processed — the case the ghost lists' slack slot absorbs.
        let mut cache = CarCache::new(2);
        cache.put("a", 1);
        cache.put("b", 2);
        cache.get(&"a");
        cache.put("c", 3); // a recirculates to T2; b demotes to B1
        cache.put("b", 20); // B1 hit: c demotes to B1, b joins T2, p = 1
        cache.put("d", 4); // a demotes to B2
        cache.put("c", 30); // B1 hit: d demotes to B1, c joins T2, p = 2
        cache.put("e", 5); // b demotes to B2; line 8 discards ghost a
        cache.put("d", 40); // B1 hit: c demotes to B2, d joins T2
        assert_eq!(cache.b1.len(), 0);
        assert_eq!(cache.b2.len(), cache.capacity());
        assert!(matches!(cache.index.get(&"b"), Some(Location::B2(_))));

        // replace() demotes T2's head (d) into the full B2 first; b's
        // own ghost must survive it for the adaptation hit to land.
        let p_before = cache.adaptation_parameter();
        cache.put("b", 100);
        assert!(
            cache.adaptation_parameter() < p_before,
            "B2 hit must adapt p downward"
        );
        // Line 19: a ghost hit promotes to T2, not T1.
        assert!(matches!(cache.index.get(&"b"), Some(Location::T2(_))));
        assert_eq!(cache.get(&"b"), Some(&100));
        assert_car_invariants(&cache);
    }

    #[test]
    fn test_zero_capacity_cache_is_disabled() {
        let mut cache = CarCache::new(0);

        assert_eq!(cache.capacity(), 0);
        assert!(cache.is_empty());
        assert!(cache.put("a", 1).is_none());
        assert!(cache.put("b", 2).is_none());
        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.adaptation_parameter(), 0);
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
        // Fills the cache; the next put replaces: `a` (referenced)
        // recirculates to T2 and `b` demotes to B1.
        cache.put("f", 6);

        // B1 hit on `b` adapts p upward.
        cache.put("b", 10);

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

        // Three B1 hits grow p to 3, forcing replacements to come from
        // T2; `a` (recirculated there with a cleared bit) demotes to B2.
        cache.put("b", 10);
        cache.put("c", 10);
        cache.put("e", 10);

        let p_before = cache.adaptation_parameter();

        // B2 hit on `a` adapts p downward.
        cache.put("a", 10);

        assert!(cache.adaptation_parameter() < p_before);
    }

    /// Full capacity-8 cache with T1 = {5..8}, T2 = {0..3} and `4` in
    /// B1: fill, reference the first half, then evict once.
    fn cache_with_mixed_directory() -> CarCache<i32, i32> {
        let mut cache = CarCache::new(8);
        for i in 0..8 {
            cache.put(i, i);
        }
        for i in 0..4 {
            cache.get(&i);
        }
        cache.put(8, 8); // 0-3 recirculate to T2; 4 demotes to B1
        cache
    }

    #[test]
    fn test_b1_adaptation_delta_scales_with_ghost_ratio() {
        // Line 15: delta = max(1, |B2|/|B1|). Plant a B2-heavy ghost
        // directory and pin the exact jump.
        let mut cache = cache_with_mixed_directory();
        for k in 200..204 {
            let slot = cache.b2.insert_at_tail(k).expect("planted ghost fits");
            cache.index.insert(k, Location::B2(slot));
        }
        assert_car_invariants(&cache);

        // replace() demotes T1's head into B1 first, so at line 15
        // |B1| = 2 and |B2| = 4: p jumps by 4/2 = 2, not by 1.
        cache.put(4, 40);
        assert_eq!(cache.adaptation_parameter(), 2);
        assert_car_invariants(&cache);
    }

    #[test]
    fn test_b2_adaptation_delta_scales_with_ghost_ratio() {
        // Line 18: delta = max(1, |B1|/|B2|), saturating at zero.
        let mut cache = cache_with_mixed_directory();
        for k in 100..103 {
            let slot = cache.b1.insert_at_tail(k).expect("planted ghost fits");
            cache.index.insert(k, Location::B1(slot));
        }
        let slot = cache.b2.insert_at_tail(200).expect("planted ghost fits");
        cache.index.insert(200, Location::B2(slot));
        cache.p = 4;
        assert_car_invariants(&cache);

        // replace() demotes T1's head into B1 first (|T1| = 4 >= p),
        // so at line 18 |B1| = 5 and |B2| = 1: p drops by 5, not 1.
        cache.put(200, 0);
        assert_eq!(cache.adaptation_parameter(), 0);
        assert_car_invariants(&cache);
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
        // The workload drives p across its full range: B1 hits push it
        // to the capacity clamp, B2 hits pull it back down.
        let peak = p_values
            .iter()
            .position(|&p| p == cache.capacity())
            .expect("B1 hits must push p to the capacity clamp");
        assert!(
            p_values[peak..].iter().any(|&p| p < cache.capacity()),
            "B2 hits must pull p back down from the clamp"
        );
    }

    #[test]
    fn test_put_return_values_eviction() {
        let mut cache = CarCache::new(3);

        assert!(cache.put("a", 1).is_none());
        assert!(cache.put("b", 2).is_none());
        assert!(cache.put("c", 3).is_none());

        // When eviction occurs, we get back the Evicted struct with key and value
        let evicted = cache.put("d", 4);
        assert!(evicted.is_some());
        let evicted = evicted.unwrap();
        assert_eq!(evicted.key, "a");
        assert_eq!(evicted.value, 1);

        let evicted = cache.put("e", 5);
        assert!(evicted.is_some());
        let evicted = evicted.unwrap();
        assert_eq!(evicted.key, "b");
        assert_eq!(evicted.value, 2);

        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"d"), Some(&4));
        assert_eq!(cache.get(&"e"), Some(&5));
    }

    #[test]
    fn test_put_return_values_t1_t2_eviction() {
        let mut cache = CarCache::new(4);

        assert!(cache.put("t1_a", 1).is_none());
        assert!(cache.put("t1_b", 2).is_none());

        cache.get(&"t1_a");
        cache.get(&"t1_b");

        assert!(cache.put("t1_c", 3).is_none());
        assert!(cache.put("t1_d", 4).is_none());

        let evicted = cache.put("new1", 10);
        assert!(evicted.is_some());
        assert_eq!(evicted.unwrap().value, 3);
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

    #[test]
    fn test_put_typed_works_across_types() {
        let mut cache: TypeErasedCarCache<String> = CarCache::new(2);

        let evicted_key = cache.put_typed("key1".to_string(), Arc::new(TypeA { id: "1".into() }));
        assert!(evicted_key.is_none());

        let evicted_key = cache.put_typed("key2".to_string(), Arc::new(TypeA { id: "2".into() }));
        assert!(evicted_key.is_none());

        let evicted_key = cache.put_typed("key3".to_string(), Arc::new(TypeB { id: "3".into() }));

        assert!(evicted_key.is_some(),);

        let evicted_key = evicted_key.unwrap();
        assert!(evicted_key == "key1" || evicted_key == "key2",);

        let key_in_cache = cache.get_typed::<Arc<TypeA>>(&evicted_key).is_some();
        assert!(!key_in_cache,);
    }
}
