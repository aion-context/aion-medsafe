// SPDX-License-Identifier: MIT OR Apache-2.0
//! Snapshot diff — what changed between two sealed signal runs.
//!
//! For a continuously-operated system (monthly data refresh, weekly re-scan),
//! the actionable output is the DELTA, not the full list. This compares two
//! sealed signal `.aion` outputs by `signal_id` and reports:
//!   - added   — newly flagged since the prior run
//!   - removed — no longer flagged (resolved / reinstated / data dropped)
//!   - changed — same signal, different confidence or queued/below status
//!
//! Both inputs are verified before parsing (no loose trust).

use std::collections::BTreeMap;
use std::path::Path;

use aion_context::key_registry::KeyRegistry;
use serde::Deserialize;

use crate::provenance;

/// Minimal view of a signal record from a sealed signal output. Non-signal
/// lines (run_meta, identity_review_candidate) lack `signal_id` and are skipped.
#[derive(Debug, Clone, Deserialize)]
pub struct SigRow {
    #[serde(default)]
    pub signal_id: String,
    #[serde(default)]
    pub signal_type: String,
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub entity_name: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub reason_code: String,
}

#[derive(Debug, Clone)]
pub struct Changed {
    pub row: SigRow,
    pub from_reason: String,
    pub from_confidence: f64,
}

#[derive(Debug, Default)]
pub struct DiffReport {
    pub added: Vec<SigRow>,
    pub removed: Vec<SigRow>,
    pub changed: Vec<Changed>,
}

fn parse_signals(payload: &[u8]) -> Vec<SigRow> {
    let mut rows = Vec::new();
    for line in payload.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if let Ok(row) = serde_json::from_slice::<SigRow>(line) {
            if !row.signal_id.is_empty() {
                rows.push(row);
            }
        }
    }
    rows
}

/// Compare two signal sets by `signal_id` (pure — directly testable).
pub fn diff_signals(from: &[SigRow], to: &[SigRow]) -> DiffReport {
    let from_map: BTreeMap<&str, &SigRow> =
        from.iter().map(|r| (r.signal_id.as_str(), r)).collect();
    let to_map: BTreeMap<&str, &SigRow> = to.iter().map(|r| (r.signal_id.as_str(), r)).collect();

    let mut report = DiffReport::default();
    for (id, row) in &to_map {
        match from_map.get(id) {
            None => report.added.push((*row).clone()),
            Some(prev) => {
                let conf_changed = (prev.confidence - row.confidence).abs() > 1e-9;
                if prev.reason_code != row.reason_code || conf_changed {
                    report.changed.push(Changed {
                        row: (*row).clone(),
                        from_reason: prev.reason_code.clone(),
                        from_confidence: prev.confidence,
                    });
                }
            }
        }
    }
    for (id, row) in &from_map {
        if !to_map.contains_key(id) {
            report.removed.push((*row).clone());
        }
    }
    report
}

/// Verify + load two sealed signal outputs and print their delta.
pub fn run(from_path: &Path, to_path: &Path) -> anyhow::Result<()> {
    let registry = provenance::load_registry(Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let from = parse_signals(&load(from_path, &registry)?);
    let to = parse_signals(&load(to_path, &registry)?);
    let report = diff_signals(&from, &to);

    println!("✓ Snapshot diff (both inputs verified)");
    println!("  From: {} ({} signals)", from_path.display(), from.len());
    println!("  To:   {} ({} signals)", to_path.display(), to.len());
    println!();
    println!("  + added (newly flagged):   {}", report.added.len());
    println!("  - removed (resolved):      {}", report.removed.len());
    println!("  ~ changed (conf/status):   {}", report.changed.len());

    print_section(
        "+ Newly flagged",
        report.added.iter().map(|r| {
            format!(
                "{} — {} ({}) [{:.2} {}]",
                r.signal_type, r.entity_name, r.entity_id, r.confidence, r.reason_code
            )
        }),
    );
    print_section(
        "- Resolved / dropped",
        report
            .removed
            .iter()
            .map(|r| format!("{} — {} ({})", r.signal_type, r.entity_name, r.entity_id)),
    );
    print_section(
        "~ Changed",
        report.changed.iter().map(|c| {
            format!(
                "{} — {} ({}): {} {:.2} -> {} {:.2}",
                c.row.signal_type,
                c.row.entity_name,
                c.row.entity_id,
                c.from_reason,
                c.from_confidence,
                c.row.reason_code,
                c.row.confidence
            )
        }),
    );
    Ok(())
}

fn load(path: &Path, registry: &KeyRegistry) -> anyhow::Result<Vec<u8>> {
    Ok(provenance::load_verified_payload(path, registry)?)
}

fn print_section(title: &str, items: impl Iterator<Item = String>) {
    let items: Vec<String> = items.collect();
    if items.is_empty() {
        return;
    }
    println!("\n  {title}:");
    for line in items.iter().take(10) {
        println!("    {line}");
    }
    if items.len() > 10 {
        println!("    … and {} more", items.len() - 10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, kind: &str, conf: f64, reason: &str) -> SigRow {
        SigRow {
            signal_id: id.to_string(),
            signal_type: kind.to_string(),
            entity_id: "E".to_string(),
            entity_name: "N".to_string(),
            confidence: conf,
            reason_code: reason.to_string(),
        }
    }

    #[test]
    fn detects_added_removed_changed() {
        let from = vec![
            row("a", "re_exclusion", 0.85, "signal_queued_review"),
            row("b", "multi_state_exclusion", 0.75, "signal_queued_review"),
        ];
        let to = vec![
            // a unchanged
            row("a", "re_exclusion", 0.85, "signal_queued_review"),
            // b dropped below threshold (status changed)
            row("b", "multi_state_exclusion", 0.65, "signal_below_threshold"),
            // c newly flagged
            row(
                "c",
                "active_npi_while_excluded",
                0.85,
                "signal_queued_review",
            ),
        ];
        let d = diff_signals(&from, &to);
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.added[0].signal_id, "c");
        assert!(d.removed.is_empty());
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].row.signal_id, "b");
        assert_eq!(d.changed[0].from_reason, "signal_queued_review");
    }

    #[test]
    fn identical_runs_have_empty_diff() {
        let rows = vec![row("a", "re_exclusion", 0.85, "signal_queued_review")];
        let d = diff_signals(&rows, &rows);
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty());
    }

    #[test]
    fn parse_skips_non_signal_lines() {
        let payload = b"{\"record\":\"run_meta\",\"signal_count\":2}\n{\"signal_id\":\"x\",\"signal_type\":\"re_exclusion\",\"confidence\":0.9,\"reason_code\":\"signal_queued_review\"}\n";
        let rows = parse_signals(payload);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].signal_id, "x");
    }
}
