// SPDX-License-Identifier: MIT OR Apache-2.0
//! Human review decisions for entity-resolution identity links.
//!
//! When a reviewer confirms or rejects an `identity_link_candidate` surfaced by
//! `/signal`, that decision is recorded here as a new version in a sealed,
//! hash-chained `.aion` (the chain IS the audit trail). On the next
//! `build-graph`, confirmed links force a merge and rejected links are
//! suppressed from the review queue.
//!
//! Each decision is signed with the reviewing analyst's OWN enrolled key
//! (author 80010+, see `enroll-analyst`), so the `.aion` signature
//! cryptographically attributes the decision to that reviewer and `load`
//! refuses anything that does not verify against the registry.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use aion_context::key_registry::KeyRegistry;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::provenance;

/// Default sealed decision log.
pub const DEFAULT_DECISIONS_PATH: &str = "decisions/identity_decisions.aion";

/// A reviewer's verdict on a candidate identity link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub entity_a: String,
    pub entity_b: String,
    /// "confirm" (same provider — merge) or "reject" (distinct — keep separate).
    pub decision: String,
    pub reviewer: String,
    #[serde(default)]
    pub reason: String,
    pub decided_at: String,
}

pub const CONFIRM: &str = "confirm";
pub const REJECT: &str = "reject";

/// Order a pair canonically so (a,b) and (b,a) are the same decision key.
pub fn canonical_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Load all decisions from the sealed log (latest verified version), or an
/// empty list if the log does not exist yet. REFUSES on verification failure.
pub fn load(path: &Path, registry: &KeyRegistry) -> Result<Vec<Decision>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let payload = provenance::load_verified_payload(path, registry)?;
    let mut decisions = Vec::new();
    for line in payload.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if let Ok(decision) = serde_json::from_slice::<Decision>(line) {
            decisions.push(decision);
        }
    }
    Ok(decisions)
}

/// Record a decision: load current, upsert (latest wins per pair), and commit a
/// new sealed version. Returns the total decision count after the write.
pub fn record(
    path: &Path,
    registry: &KeyRegistry,
    author_id: u64,
    signing_key: &aion_context::crypto::SigningKey,
    decision: Decision,
) -> Result<usize> {
    // Latest-per-pair: a new verdict supersedes any prior one for the same pair.
    let mut by_pair: BTreeMap<(String, String), Decision> = BTreeMap::new();
    for existing in load(path, registry)? {
        by_pair.insert(
            canonical_pair(&existing.entity_a, &existing.entity_b),
            existing,
        );
    }
    by_pair.insert(
        canonical_pair(&decision.entity_a, &decision.entity_b),
        decision,
    );

    let mut payload = String::new();
    for decision in by_pair.values() {
        if let Ok(line) = serde_json::to_string(decision) {
            payload.push_str(&line);
            payload.push('\n');
        }
    }

    provenance::commit_payload(
        path,
        payload.as_bytes(),
        author_id,
        signing_key,
        registry,
        "Record identity-link review decision",
    )?;
    Ok(by_pair.len())
}

/// The set of confirmed and rejected canonical pairs (latest verdict per pair).
pub struct Verdicts {
    pub confirmed: BTreeSet<(String, String)>,
    pub rejected: BTreeSet<(String, String)>,
}

pub fn verdicts(decisions: &[Decision]) -> Verdicts {
    // Latest entry per pair wins (decisions are appended in order).
    let mut latest: BTreeMap<(String, String), &Decision> = BTreeMap::new();
    for decision in decisions {
        latest.insert(
            canonical_pair(&decision.entity_a, &decision.entity_b),
            decision,
        );
    }
    let mut confirmed = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    for (pair, decision) in latest {
        match decision.decision.as_str() {
            CONFIRM => {
                confirmed.insert(pair);
            }
            REJECT => {
                rejected.insert(pair);
            }
            _ => {}
        }
    }
    Verdicts {
        confirmed,
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_context::crypto::SigningKey;
    use aion_context::types::AuthorId;

    fn d(a: &str, b: &str, decision: &str) -> Decision {
        Decision {
            entity_a: a.to_string(),
            entity_b: b.to_string(),
            decision: decision.to_string(),
            reviewer: "analyst".to_string(),
            reason: String::new(),
            decided_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn canonical_pair_is_order_independent() {
        assert_eq!(canonical_pair("B", "A"), canonical_pair("A", "B"));
    }

    #[test]
    fn latest_verdict_per_pair_wins() {
        let decisions = vec![d("E1", "E2", CONFIRM), d("E2", "E1", REJECT)];
        let v = verdicts(&decisions);
        assert!(v.rejected.contains(&canonical_pair("E1", "E2")));
        assert!(!v.confirmed.contains(&canonical_pair("E1", "E2")));
    }

    #[test]
    fn splits_confirmed_and_rejected() {
        let decisions = vec![d("A", "B", CONFIRM), d("C", "D", REJECT)];
        let v = verdicts(&decisions);
        assert_eq!(v.confirmed.len(), 1);
        assert_eq!(v.rejected.len(), 1);
    }

    #[test]
    fn record_signs_and_appends_a_verified_chain() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("decisions.aion");

        // Registry with an enrolled analyst (author 80010).
        let key = SigningKey::generate();
        let mut registry = KeyRegistry::new();
        registry
            .register_author(
                AuthorId::new(80010),
                key.verifying_key(),
                key.verifying_key(),
                0,
            )
            .expect("register analyst");

        let total = record(&path, &registry, 80010, &key, d("E1", "E2", CONFIRM)).expect("record");
        assert_eq!(total, 1);

        // Second decision appends a new sealed version to the chain.
        record(&path, &registry, 80010, &key, d("E3", "E4", REJECT)).expect("record 2");
        let loaded = load(&path, &registry).expect("load");
        assert_eq!(loaded.len(), 2);
        let v = verdicts(&loaded);
        assert_eq!(v.confirmed.len(), 1);
        assert_eq!(v.rejected.len(), 1);
    }
}
