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

//! Instruction-count benchmarks for the CAR cache (gungraun / Valgrind
//! Callgrind).
//!
//! Keys are `Url`s like production (`HttpBmc` keys its response cache by
//! request URL), so key hashing costs are realistic; values are plain
//! `u64` to isolate the algorithm from the `Box<dyn Any>` type erasure.
//! Caches are built in setup (excluded from measurement) and returned
//! from the benchmark so their drop stays outside it. Valgrind is
//! unix-only, so the whole benchmark is `cfg(unix)`.
//!
//! Slot storage allocates lazily on first use, so benchmarks that
//! perform a cache's first eviction (evict_first_probe,
//! evict_full_clock_sweep) include that one-time allocation;
//! steady-state evictions cost less by that share.

#[cfg(unix)]
mod unix {
    use std::hint::black_box;

    use gungraun::library_benchmark;
    use nv_redfish_bmc_http::cache::CarCache;
    use rustc_hash::FxBuildHasher;
    use url::Url;

    const CAPACITY: usize = 1024;

    type Cache = CarCache<Url, u64, FxBuildHasher>;

    fn key(i: usize) -> Url {
        let url = format!("https://bmc.local/redfish/v1/Systems/{i}/Sensors/{i}");
        Url::parse(&url).expect("static test URL parses")
    }

    /// Cache filled to capacity; every entry in T1 with a clear
    /// reference bit, so eviction finds a victim on the first probe.
    fn full(c: usize) -> Cache {
        let mut cache = Cache::with_hasher(c, FxBuildHasher);
        for i in 0..c {
            cache.put(key(i), i as u64);
        }
        cache
    }

    /// Cache filled to half its capacity, so insertion takes the
    /// no-eviction path. Fills one entry past c/2 so the measured
    /// insert doesn't land on the slot vector's doubling boundary
    /// and pay its reallocation.
    fn half_full(c: usize) -> Cache {
        let mut cache = Cache::with_hasher(c, FxBuildHasher);
        for i in 0..=c / 2 {
            cache.put(key(i), i as u64);
        }
        cache
    }

    /// Like [`full`], but every entry's reference bit is set: eviction
    /// must sweep the whole clock once, demoting entries to T2.
    fn full_referenced(c: usize) -> Cache {
        let mut cache = full(c);
        for i in 0..c {
            black_box(cache.get(&key(i)));
        }
        cache
    }

    /// Full cache whose ghost list B1 deterministically holds evicted
    /// keys: the referenced half of the population recirculates to T2
    /// during evictions, so the unreferenced half demotes to B1 and
    /// survives (T2 keeps |T1| + |B1| below the discard threshold).
    /// Returns the cache and a key proven to be a B1 ghost.
    fn ghost_input(c: usize) -> (Cache, Url) {
        let build = || {
            let mut cache = full(c);
            for i in 0..c / 2 {
                black_box(cache.get(&key(i)));
            }
            for i in c..c + c / 2 {
                cache.put(key(i), i as u64);
            }
            cache
        };
        // Prove the measured put takes the B1 adaptation path: a B1
        // hit must raise the adaptation parameter.
        let ghost = key(c - 1);
        let mut probe = build();
        let p_before = probe.adaptation_parameter();
        probe.put(ghost.clone(), 0);
        assert!(
            probe.adaptation_parameter() > p_before,
            "setup must produce a B1 ghost hit"
        );
        (build(), ghost)
    }

    #[library_benchmark]
    #[bench::hit((full(CAPACITY), key(0)))]
    #[bench::miss((full(CAPACITY), key(CAPACITY + 1)))]
    fn get((mut cache, k): (Cache, Url)) -> (Cache, Option<u64>) {
        let value = cache.get(&k).copied();
        (cache, black_box(value))
    }

    #[library_benchmark]
    #[bench::update_existing((full(CAPACITY), key(0)))]
    #[bench::insert_with_headroom((half_full(CAPACITY), key(CAPACITY)))]
    #[bench::evict_first_probe((full(CAPACITY), key(CAPACITY + 1)))]
    #[bench::evict_full_clock_sweep((full_referenced(CAPACITY), key(CAPACITY + 1)))]
    #[bench::ghost_hit(ghost_input(CAPACITY))]
    fn put((mut cache, k): (Cache, Url)) -> Cache {
        black_box(cache.put(k, 7));
        cache
    }

    fn scan_input(c: usize) -> (Cache, Vec<Url>) {
        let keys = (0..256).map(|i| key(2 * c + i)).collect();
        (full_referenced(c), keys)
    }

    // Workload-shaped batch: a scan of 256 never-before-seen keys
    // through a full, referenced cache (the pattern a Redfish resource
    // walk produces). Keys are pre-built in setup so only cache
    // operations are measured.
    #[library_benchmark]
    #[bench::n_256(scan_input(CAPACITY))]
    fn scan((mut cache, keys): (Cache, Vec<Url>)) -> Cache {
        for (i, k) in keys.into_iter().enumerate() {
            black_box(cache.put(k, i as u64));
        }
        cache
    }
}

#[cfg(unix)]
use unix::{get, put, scan};

#[cfg(unix)]
gungraun::library_benchmark_group!(
    name = cache;
    benchmarks = get, put, scan
);

#[cfg(unix)]
gungraun::main!(library_benchmark_groups = cache);

#[cfg(not(unix))]
fn main() {}
