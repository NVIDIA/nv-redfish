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

//! Generic helpers used by `build.rs` of nv-redfish workspace crates.

use std::env;
use std::panic::resume_unwind;
use std::path::Path;
use std::path::PathBuf;
use std::thread;

/// 16 MB stack size used for codegen worker threads.
///
/// Required for deep CSDL type hierarchies on platforms with small default
/// stacks (notably Windows, where the default is 1 MB).
const BUILD_STACK_SIZE: usize = 16 * 1024 * 1024;

/// Run `f` on a worker thread with a 16 MB stack and propagate panics.
///
/// Errors are stringified into the `Err` variant returned by `main`. The
/// closure must be `Send` because it is moved to a freshly spawned thread.
///
/// # Panics
///
/// Panics if the worker thread cannot be spawned. Any panic raised inside
/// `f` is re-raised on the caller's thread via [`resume_unwind`].
pub fn run_with_big_stack<F, E>(f: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), E> + Send + 'static,
    E: std::fmt::Debug + 'static,
{
    thread::Builder::new()
        .stack_size(BUILD_STACK_SIZE)
        .spawn(move || f().map_err(|err| format!("{err:#?}")))
        .expect("failed to spawn build thread")
        .join()
        .unwrap_or_else(|payload| resume_unwind(payload))
}

/// Returns the value of the `OUT_DIR` environment variable Cargo sets for
/// build scripts.
///
/// # Panics
///
/// Panics if `OUT_DIR` is not set. Cargo always sets it for build scripts,
/// so this only fires if the function is misused outside that context.
#[must_use]
pub fn out_dir() -> PathBuf {
    PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set by Cargo"))
}

/// Emit `cargo:rerun-if-changed` for every path in `paths`.
pub fn rerun_for<I, P>(paths: I)
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.as_ref().display());
    }
}

/// Returns `true` iff Cargo set `CARGO_FEATURE_<NAME>` for the current build.
///
/// The provided `name` is uppercased and `-` is replaced with `_` to match
/// Cargo's env-var naming convention.
#[must_use]
pub fn cargo_feature_enabled(name: &str) -> bool {
    env::var(format!(
        "CARGO_FEATURE_{}",
        name.to_uppercase().replace('-', "_")
    ))
    .is_ok()
}
