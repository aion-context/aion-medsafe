// SPDX-License-Identifier: MIT OR Apache-2.0
//! Signal adjudication + calibration — closing the feedback loop.
//!
//! After a human reviews a flagged provider, they record whether the signal was
//! a TRUE or FALSE positive. Those verdicts are appended to a sealed,
//! hash-chained, per-analyst-signed `.aion` (the same chain-of-custody pattern
//! as the identity-decision log). `calibrate` then tallies them into per-signal-
//! type precision — turning the engine's asserted confidence into *earned*
//! precision that the next run surfaces alongside each signal.

use std::collections::BTreeMap;
use std::path::Path;

use aion_context::key_registry::KeyRegistry;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::provenance;

/// Default sealed adjudication log.
pub const DEFAULT_ADJUDICATIONS_PATH: &str = "decisions/signal_adjudications.aion";

pub const TRUE_POSITIVE: &str = "true_positive";
pub const FALSE_POSITIVE: &str = "false_positive";

/// A reviewer's verdict on a fired signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adjudication {
    pub signal_id: String,
    pub signal_type: String,
    pub entity_id: String,
    /// "true_positive" or "false_positive".
    pub verdict: String,
    pub reviewer: String,
    #[serde(default)]
    pub reason: String,
    pub adjudicated_at: String,
}

/// Per-signal-type tally of adjudicated outcomes.
#[derive(Debug, Clone, Default)]
pub struct TypeStats {
    pub tp: u32,
    pub fp: u32,
}

impl TypeStats {
    pub fn total(&self) -> u32 {
        self.tp + self.fp
    }
    /// Earned precision (tp / total), or None if nothing adjudicated yet.
    pub fn precision(&self) -> Option<f64> {
        let n = self.total();
        if n == 0 {
            None
        } else {
            Some(self.tp as f64 / n as f64)
        }
    }
}

/// Load all adjudications from the sealed log (verified), or empty if none.
pub fn load(path: &Path, registry: &KeyRegistry) -> Result<Vec<Adjudication>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let payload = provenance::load_verified_payload(path, registry)?;
    let mut out = Vec::new();
    for line in payload.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if let Ok(a) = serde_json::from_slice::<Adjudication>(line) {
            out.push(a);
        }
    }
    Ok(out)
}

/// Record a verdict: load current, upsert by `signal_id` (latest wins), and
/// commit a new sealed version signed by the reviewing analyst.
pub fn record(
    path: &Path,
    registry: &KeyRegistry,
    author_id: u64,
    signing_key: &aion_context::crypto::SigningKey,
    adjudication: Adjudication,
) -> Result<usize> {
    let mut by_signal: BTreeMap<String, Adjudication> = BTreeMap::new();
    for existing in load(path, registry)? {
        by_signal.insert(existing.signal_id.clone(), existing);
    }
    by_signal.insert(adjudication.signal_id.clone(), adjudication);

    let mut payload = String::new();
    for a in by_signal.values() {
        if let Ok(line) = serde_json::to_string(a) {
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
        "Record signal adjudication",
    )?;
    Ok(by_signal.len())
}

/// Tally per-signal-type precision (latest verdict per signal_id wins).
pub fn calibrate(adjudications: &[Adjudication]) -> BTreeMap<String, TypeStats> {
    let mut latest: BTreeMap<&str, &Adjudication> = BTreeMap::new();
    for a in adjudications {
        latest.insert(&a.signal_id, a);
    }
    let mut stats: BTreeMap<String, TypeStats> = BTreeMap::new();
    for a in latest.values() {
        let entry = stats.entry(a.signal_type.clone()).or_default();
        match a.verdict.as_str() {
            TRUE_POSITIVE => entry.tp += 1,
            FALSE_POSITIVE => entry.fp += 1,
            _ => {}
        }
    }
    stats
}

/// Annotate signals with their signal type's earned precision (if adjudicated).
pub fn annotate(
    signals: &mut [crate::detection::ComputedSignal],
    cal: &BTreeMap<String, TypeStats>,
) {
    for s in signals.iter_mut() {
        s.calibrated_precision = cal.get(&s.signal_type).and_then(TypeStats::precision);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adj(signal_id: &str, kind: &str, verdict: &str) -> Adjudication {
        Adjudication {
            signal_id: signal_id.to_string(),
            signal_type: kind.to_string(),
            entity_id: "E1".to_string(),
            verdict: verdict.to_string(),
            reviewer: "80010".to_string(),
            reason: String::new(),
            adjudicated_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn precision_per_type() {
        let a = vec![
            adj("s1", "re_exclusion", TRUE_POSITIVE),
            adj("s2", "re_exclusion", TRUE_POSITIVE),
            adj("s3", "re_exclusion", FALSE_POSITIVE),
            adj("s4", "multi_state_exclusion", FALSE_POSITIVE),
        ];
        let cal = calibrate(&a);
        assert!((cal["re_exclusion"].precision().unwrap() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(cal["multi_state_exclusion"].precision().unwrap(), 0.0);
    }

    #[test]
    fn latest_verdict_per_signal_wins() {
        let a = vec![
            adj("s1", "re_exclusion", FALSE_POSITIVE),
            adj("s1", "re_exclusion", TRUE_POSITIVE), // reviewer corrected
        ];
        let cal = calibrate(&a);
        assert_eq!(cal["re_exclusion"].tp, 1);
        assert_eq!(cal["re_exclusion"].fp, 0);
    }
}
