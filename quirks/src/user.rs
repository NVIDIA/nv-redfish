// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Repairs supplied at runtime by the caller, applied after the
//! platform's own. The set is shared by every clone of the layer that
//! holds it and can be replaced while reads are in flight: a read takes
//! the set as it stands when the document arrives.

use std::fmt;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use serde_json::Value;

/// A caller's document rewrite.
pub type UserFix = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Which documents a [`UserRule`] applies to.
#[derive(Debug)]
pub enum Match {
    /// Documents whose `@odata.type` family is this, e.g.
    /// `"ManagerNetworkProtocol"` for
    /// `#ManagerNetworkProtocol.v1_9_0.ManagerNetworkProtocol`.
    Type(String),
    /// Documents whose `@odata.id` matches this pattern.
    Id(IdPattern),
}

impl Match {
    fn applies(&self, resource_type: Option<&str>, odata_id: Option<&str>) -> bool {
        match self {
            Self::Type(family) => resource_type == Some(family.as_str()),
            Self::Id(pattern) => odata_id.is_some_and(|id| pattern.matches(id)),
        }
    }
}

/// An `@odata.id` pattern: path segments compared one by one, where a `*`
/// segment matches any single segment.
///
/// `/redfish/v1/Managers/*/NetworkProtocol` matches
/// `/redfish/v1/Managers/BMC/NetworkProtocol` but not
/// `/redfish/v1/Managers/BMC/Extra/NetworkProtocol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdPattern(Vec<String>);

impl IdPattern {
    /// The pattern `pattern`.
    #[must_use]
    pub fn new(pattern: &str) -> Self {
        Self(pattern.split('/').map(str::to_owned).collect())
    }

    /// Whether `odata_id` matches.
    #[must_use]
    pub fn matches(&self, odata_id: &str) -> bool {
        let mut segments = odata_id.split('/');
        self.0.iter().all(|expected| {
            segments
                .next()
                .is_some_and(|s| expected == "*" || expected == s)
        }) && segments.next().is_none()
    }
}

/// A named repair for the documents [`Match`] selects.
pub struct UserRule {
    name: String,
    matches: Match,
    fix: UserFix,
}

impl UserRule {
    /// The rule `name` that applies `fix` to the documents `matches`
    /// selects.
    pub fn new(
        name: impl Into<String>,
        matches: Match,
        fix: impl Fn(Value) -> Value + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            matches,
            fix: Arc::new(fix),
        }
    }

    /// The rule's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn applies(&self, resource_type: Option<&str>, odata_id: Option<&str>) -> bool {
        self.matches.applies(resource_type, odata_id)
    }

    pub(crate) fn apply(&self, value: Value) -> Value {
        (self.fix)(value)
    }
}

impl fmt::Debug for UserRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserRule")
            .field("name", &self.name)
            .field("matches", &self.matches)
            .finish_non_exhaustive()
    }
}

/// The caller's rule set, shared by every clone of this handle.
#[derive(Clone, Default)]
pub struct UserRules(Arc<RwLock<Arc<[UserRule]>>>);

impl UserRules {
    /// Replaces the rule set; rules apply in the order given. Reads already
    /// repairing finish with the set they started with.
    pub fn replace(&self, rules: Vec<UserRule>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = rules.into();
    }

    /// Removes every rule.
    pub fn clear(&self) {
        self.replace(Vec::new());
    }

    /// The rule set as it stands.
    #[must_use]
    pub fn snapshot(&self) -> Arc<[UserRule]> {
        Arc::clone(&self.0.read().unwrap_or_else(PoisonError::into_inner))
    }
}

impl fmt::Debug for UserRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.snapshot().iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_id_pattern_matches_segment_by_segment() {
        let pattern = IdPattern::new("/redfish/v1/Managers/*/NetworkProtocol");
        assert!(pattern.matches("/redfish/v1/Managers/BMC/NetworkProtocol"));
        assert!(!pattern.matches("/redfish/v1/Managers/BMC/Extra/NetworkProtocol"));
        assert!(!pattern.matches("/redfish/v1/Managers/BMC"));
        assert!(!pattern.matches("/redfish/v1/Managers/BMC/NetworkProtocol/More"));
    }

    #[test]
    fn a_replaced_set_is_seen_by_every_clone() {
        let rules = UserRules::default();
        let shared = rules.clone();
        assert!(shared.snapshot().is_empty());

        rules.replace(vec![UserRule::new(
            "name",
            Match::Type("Chassis".into()),
            |mut v| {
                v["Name"] = json!("named");
                v
            },
        )]);
        let snapshot = shared.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].applies(Some("Chassis"), None));
        assert!(!snapshot[0].applies(Some("ChassisCollection"), None));
        assert_eq!(snapshot[0].apply(json!({}))["Name"], "named");

        // A snapshot taken before a replacement keeps its rules.
        rules.clear();
        assert_eq!(snapshot.len(), 1);
        assert!(shared.snapshot().is_empty());
    }
}
