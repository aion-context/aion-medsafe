// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk signal detection engine.
//!
//! Pure functions over a verified [`TrustGraph`] and a verified
//! [`DetectionPolicy`]. Detectors NEVER accuse — they emit evidence-ranked
//! signals with a confidence score, then a policy-driven threshold decides
//! whether each signal is queued for human review or archived below threshold
//! (see `.claude/rules/agents.md`). Signal types the available data cannot
//! support (e.g. `billing_after_exclusion`, which needs claims data) are
//! reported as not-computable rather than fabricated.

use std::collections::BTreeSet;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::graph::{Entity, ExclusionAuthority, ExclusionEvent, ExclusionStatus, TrustGraph};
use crate::policy::DetectionPolicy;

/// Bounded reason-code vocabulary (subset of `.claude/rules/agents.md`).
const REASON_QUEUED_REVIEW: &str = "signal_queued_review";
const REASON_BELOW_THRESHOLD: &str = "signal_below_threshold";
const REASON_IDENTITY_REVIEW: &str = "identity_review_candidate";

/// Signal types this engine can compute from exclusion + identity data alone.
const COMPUTABLE: &[&str] = &[
    "re_exclusion",
    "multi_state_exclusion",
    "federal_state_mismatch",
    "active_npi_while_excluded",
];

/// A computed risk signal. NOT an accusation — an evidence-ranked indicator
/// that warrants human review. Serialized into the sealed signal output.
#[derive(Debug, Clone, Serialize)]
pub struct ComputedSignal {
    pub signal_id: String,
    pub entity_id: String,
    pub entity_name: String,
    pub signal_type: String,
    pub severity: f64,
    pub confidence: f64,
    pub description: String,
    pub evidence: Vec<String>,
    pub requires_human_review: bool,
    pub reason_code: String,
    pub jurisdiction: Option<String>,
}

/// Outcome of a detection run.
#[derive(Debug, Clone, Default)]
pub struct DetectionReport {
    pub signals: Vec<ComputedSignal>,
    pub not_computable: Vec<String>,
    pub entities_evaluated: usize,
}

/// An entity-resolution review item: two entities entity resolution thinks may
/// be the same provider but did not auto-merge. Surfaced for a human to confirm
/// or reject — never merged autonomously (agents.md: no auto-link below
/// threshold). Confirming a link can change which signals fire, so reviewers
/// triage these alongside risk signals.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewCandidate {
    pub entity_a: String,
    pub entity_b: String,
    pub name_a: String,
    pub name_b: String,
    pub confidence: f64,
    pub signals: Vec<String>,
    pub reason_code: String,
}

/// Build the identity-link review queue for a jurisdiction, highest confidence
/// first. A candidate is included when either endpoint is an entity with a
/// nexus to `jurisdiction` (or always, when no jurisdiction is given).
pub fn review_queue(graph: &TrustGraph, jurisdiction: Option<&str>) -> Vec<ReviewCandidate> {
    let by_id: HashMap<&str, &Entity> = graph
        .entities
        .iter()
        .map(|e| (e.entity_id.as_str(), e))
        .collect();
    let jur = jurisdiction.map(str::to_uppercase);

    let in_jur = |entity: Option<&&Entity>| -> bool {
        match (&jur, entity) {
            (None, _) => true,
            (Some(j), Some(e)) => {
                e.canonical_state.as_deref().map(str::to_uppercase) == Some(j.clone())
            }
            (Some(_), None) => false,
        }
    };

    let mut queue: Vec<ReviewCandidate> = graph
        .link_candidates
        .iter()
        .filter_map(|c| {
            let a = by_id.get(c.entity_a.as_str());
            let b = by_id.get(c.entity_b.as_str());
            if !in_jur(a) && !in_jur(b) {
                return None;
            }
            Some(ReviewCandidate {
                entity_a: c.entity_a.clone(),
                entity_b: c.entity_b.clone(),
                name_a: a.map(|e| e.canonical_name.clone()).unwrap_or_default(),
                name_b: b.map(|e| e.canonical_name.clone()).unwrap_or_default(),
                confidence: c.confidence,
                signals: c.signals.clone(),
                reason_code: REASON_IDENTITY_REVIEW.to_string(),
            })
        })
        .collect();
    queue.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    tracing::info!(
        event = "identity_review_queue_built",
        jurisdiction = jurisdiction.unwrap_or("national"),
        candidates = queue.len(),
    );
    queue
}

/// Raw detector output before policy/threshold dressing.
type Hit = (f64, String, Vec<String>);

/// Compute all policy-defined signals over the graph.
///
/// When `jurisdiction` is set, only entities with a nexus to that state are
/// evaluated, and the jurisdiction-relative `federal_state_mismatch` detector
/// is enabled.
pub fn compute(
    graph: &TrustGraph,
    policy: &DetectionPolicy,
    jurisdiction: Option<&str>,
) -> DetectionReport {
    let jur = jurisdiction.map(str::to_uppercase);
    let threshold = policy.thresholds.minimum_confidence_for_alert;
    let mut report = DetectionReport::default();

    for entity in &graph.entities {
        let events = graph.events_for(&entity.entity_id);
        if let Some(j) = jur.as_deref() {
            if !entity_in_jurisdiction(entity, events, j) {
                continue;
            }
        }
        report.entities_evaluated += 1;

        for (stype, hit) in run_detectors(entity, events, policy, jur.is_some()) {
            report
                .signals
                .push(dress(policy, entity, stype, hit, threshold, &jur));
        }
    }

    report.not_computable = policy
        .risk_signals
        .keys()
        .filter(|k| !COMPUTABLE.contains(&k.as_str()))
        .cloned()
        .collect();
    report.not_computable.sort();

    tracing::info!(
        event = "signals_computed",
        entities_evaluated = report.entities_evaluated,
        signals = report.signals.len(),
        not_computable = ?report.not_computable,
    );
    report
}

/// Run every policy-enabled detector for one entity.
fn run_detectors(
    entity: &Entity,
    events: &[ExclusionEvent],
    policy: &DetectionPolicy,
    jurisdiction_scoped: bool,
) -> Vec<(&'static str, Hit)> {
    let mut hits: Vec<(&'static str, Hit)> = Vec::new();

    if policy.has_signal("re_exclusion") {
        if let Some(h) = detect_re_exclusion(events) {
            hits.push(("re_exclusion", h));
        }
    }
    if policy.has_signal("multi_state_exclusion") {
        if let Some(h) = detect_multi_state(events) {
            hits.push(("multi_state_exclusion", h));
        }
    }
    // Federal/state mismatch is only meaningful within a state jurisdiction.
    if jurisdiction_scoped && policy.has_signal("federal_state_mismatch") {
        if let Some(h) = detect_fed_state_mismatch(events) {
            hits.push(("federal_state_mismatch", h));
        }
    }
    if policy.has_signal("active_npi_while_excluded") {
        if let Some(h) = detect_active_npi(entity, events) {
            hits.push(("active_npi_while_excluded", h));
        }
    }

    hits
}

/// Apply policy severity / review requirement and the threshold decision.
fn dress(
    policy: &DetectionPolicy,
    entity: &Entity,
    signal_type: &str,
    hit: Hit,
    threshold: f64,
    jurisdiction: &Option<String>,
) -> ComputedSignal {
    let (confidence, description, evidence) = hit;
    let reason_code = if confidence >= threshold {
        REASON_QUEUED_REVIEW
    } else {
        REASON_BELOW_THRESHOLD
    };
    ComputedSignal {
        signal_id: signal_id(signal_type, &entity.entity_id),
        entity_id: entity.entity_id.clone(),
        entity_name: entity.canonical_name.clone(),
        signal_type: signal_type.to_string(),
        severity: policy.severity(signal_type).unwrap_or(0.5),
        confidence,
        description,
        evidence,
        requires_human_review: policy.requires_review(signal_type),
        reason_code: reason_code.to_string(),
        jurisdiction: jurisdiction.clone(),
    }
}

fn signal_id(signal_type: &str, entity_id: &str) -> String {
    let digest = blake3::hash(format!("{signal_type}:{entity_id}").as_bytes());
    hex::encode(&digest.as_bytes()[..16])
}

fn entity_in_jurisdiction(entity: &Entity, events: &[ExclusionEvent], jur: &str) -> bool {
    if entity.canonical_state.as_deref().map(str::to_uppercase) == Some(jur.to_string()) {
        return true;
    }
    events
        .iter()
        .any(|e| e.state.as_deref().map(str::to_uppercase) == Some(jur.to_string()))
}

// ─── Detectors ──────────────────────────────────────────────────────────────

/// Excluded → reinstated → excluded again: a persistent-problem pattern.
fn detect_re_exclusion(events: &[ExclusionEvent]) -> Option<Hit> {
    let exclusions: Vec<&ExclusionEvent> = events
        .iter()
        .filter(|e| e.exclusion_date.is_some())
        .collect();
    let reinstatements = events
        .iter()
        .filter(|e| e.reinstatement_date.is_some())
        .count();

    if exclusions.len() > 1 && reinstatements >= 1 {
        let confidence = (0.75 + 0.05 * (exclusions.len() as f64 - 2.0)).min(1.0);
        let description = format!(
            "{} exclusion events with {} reinstatement(s): excluded, reinstated, then excluded again",
            exclusions.len(),
            reinstatements
        );
        let evidence = exclusions.iter().map(|e| e.event_id.clone()).collect();
        Some((confidence, description, evidence))
    } else {
        None
    }
}

/// Exclusions spanning multiple states — possible cross-state evasion.
fn detect_multi_state(events: &[ExclusionEvent]) -> Option<Hit> {
    let states: BTreeSet<String> = events
        .iter()
        .filter_map(|e| e.state.as_deref().map(str::to_uppercase))
        .collect();

    if states.len() >= 2 {
        let confidence = (0.6 + 0.15 * (states.len() as f64 - 1.0)).min(1.0);
        let listed: Vec<String> = states.iter().cloned().collect();
        let description = format!(
            "exclusions span {} states: {}",
            states.len(),
            listed.join(", ")
        );
        let evidence = events
            .iter()
            .filter(|e| e.state.is_some())
            .map(|e| e.event_id.clone())
            .collect();
        Some((confidence, description, evidence))
    } else {
        None
    }
}

/// On the federal LEIE but absent from the state Medicaid exclusion list.
fn detect_fed_state_mismatch(events: &[ExclusionEvent]) -> Option<Hit> {
    let federal: Vec<&ExclusionEvent> = events
        .iter()
        .filter(|e| e.authority == ExclusionAuthority::HhsOig)
        .collect();
    if federal.is_empty() {
        return None;
    }
    let has_state_medicaid = events
        .iter()
        .any(|e| e.authority == ExclusionAuthority::StateMedicaid);
    if has_state_medicaid {
        return None;
    }

    let description =
        "on federal HHS-OIG exclusion list but absent from the state Medicaid exclusion list"
            .to_string();
    let evidence = federal.iter().map(|e| e.event_id.clone()).collect();
    Some((0.7, description, evidence))
}

/// NPI still active in NPPES while the provider is under active exclusion.
fn detect_active_npi(entity: &Entity, events: &[ExclusionEvent]) -> Option<Hit> {
    if entity.npi_active != Some(true) || !currently_excluded(events) {
        return None;
    }
    let description = format!(
        "NPI active in NPPES while under active exclusion (npis: {})",
        entity.npis.join(", ")
    );
    let evidence = events
        .iter()
        .filter(|e| e.exclusion_date.is_some())
        .map(|e| e.event_id.clone())
        .collect();
    Some((0.85, description, evidence))
}

/// True if the latest exclusion is more recent than the latest reinstatement,
/// or any event is marked indefinite.
fn currently_excluded(events: &[ExclusionEvent]) -> bool {
    if events
        .iter()
        .any(|e| e.status == ExclusionStatus::Indefinite)
    {
        return true;
    }
    let latest_excl = events
        .iter()
        .filter_map(|e| parse_date(&e.exclusion_date))
        .max();
    let latest_reinst = events
        .iter()
        .filter_map(|e| parse_date(&e.reinstatement_date))
        .max();
    match (latest_excl, latest_reinst) {
        (Some(excl), Some(reinst)) => excl > reinst,
        (Some(_), None) => true,
        _ => false,
    }
}

fn parse_date(value: &Option<String>) -> Option<DateTime<Utc>> {
    let raw = value.as_deref()?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_YAML: &str = r#"
version: "1"
effective_date: "2026-06-06"
thresholds:
  minimum_confidence_for_alert: 0.70
  maximum_days_lookback: 3650
jurisdictions:
  primary: "HI"
  scope: "test"
risk_signals:
  re_exclusion: {severity: 0.85, description: "x", requires_human_review: true}
  multi_state_exclusion: {severity: 0.70, description: "x", requires_human_review: true}
  federal_state_mismatch: {severity: 0.55, description: "x", requires_human_review: true}
  active_npi_while_excluded: {severity: 0.90, description: "x", requires_human_review: true}
  billing_after_exclusion: {severity: 1.0, description: "x", requires_human_review: true}
"#;

    fn policy() -> DetectionPolicy {
        serde_yaml::from_str(POLICY_YAML).expect("parse test policy")
    }

    fn graph(lines: &[&str]) -> TrustGraph {
        TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "test").expect("parse graph")
    }

    fn entity(id: &str, state: &str) -> String {
        format!(
            r#"{{"kind":"entity","entity_id":"{id}","entity_type":"individual","canonical_name":"{id}","canonical_state":"{state}"}}"#
        )
    }

    fn excl(id: &str, entity: &str, authority: &str, state: &str, date: &str) -> String {
        format!(
            r#"{{"kind":"exclusion_event","event_id":"{id}","entity_id":"{entity}","authority":"{authority}","exclusion_date":"{date}","status":"active","state":"{state}","source_id":"s","source_record_id":"r","source_snapshot_hash":"h","observed_at":"{date}"}}"#
        )
    }

    fn reinst(id: &str, entity: &str, authority: &str, state: &str, date: &str) -> String {
        format!(
            r#"{{"kind":"exclusion_event","event_id":"{id}","entity_id":"{entity}","authority":"{authority}","reinstatement_date":"{date}","status":"reinstated","state":"{state}","source_id":"s","source_record_id":"r","source_snapshot_hash":"h","observed_at":"{date}"}}"#
        )
    }

    fn cand(a: &str, b: &str, conf: f64) -> String {
        format!(
            r#"{{"kind":"identity_link_candidate","entity_a":"{a}","entity_b":"{b}","confidence":{conf},"signals":["name_edit_distance"]}}"#
        )
    }

    fn find<'a>(r: &'a DetectionReport, kind: &str) -> Option<&'a ComputedSignal> {
        r.signals.iter().find(|s| s.signal_type == kind)
    }

    #[test]
    fn review_queue_filters_by_jurisdiction_and_sorts() {
        let g = graph(&[
            &entity("E1", "HI"),
            &entity("E2", "HI"),
            &entity("E3", "CA"),
            &entity("E4", "CA"),
            &cand("E1", "E2", 0.80), // both HI
            &cand("E3", "E4", 0.95), // both CA — excluded under HI
            &cand("E1", "E3", 0.85), // mixed — included (E1 in HI)
        ]);
        let q = review_queue(&g, Some("HI"));
        assert_eq!(q.len(), 2);
        // highest confidence first
        assert_eq!(q[0].confidence, 0.85);
        assert!(q[0].confidence >= q[1].confidence);
        assert!(q.iter().all(|c| c.reason_code == REASON_IDENTITY_REVIEW));
        // names resolved from entities
        assert_eq!(q[1].name_a, "E1");
    }

    #[test]
    fn review_queue_national_includes_all() {
        let g = graph(&[
            &entity("E1", "HI"),
            &entity("E3", "CA"),
            &cand("E1", "E3", 0.8),
        ]);
        assert_eq!(review_queue(&g, None).len(), 1);
    }

    // ── re_exclusion ────────────────────────────────────────────────────────

    #[test]
    fn re_exclusion_true_positive() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
            &reinst("b", "E1", "hhs_oig", "HI", "2018-01-01T00:00:00Z"),
            &excl("c", "E1", "hhs_oig", "HI", "2021-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        let sig = find(&r, "re_exclusion").expect("re_exclusion fires");
        assert!(sig.confidence >= 0.75);
        assert_eq!(sig.reason_code, REASON_QUEUED_REVIEW);
        assert!(sig.requires_human_review);
        assert_eq!(sig.severity, 0.85);
    }

    #[test]
    fn re_exclusion_true_negative_single_exclusion() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(find(&r, "re_exclusion").is_none());
    }

    // ── multi_state_exclusion ───────────────────────────────────────────────

    #[test]
    fn multi_state_true_positive() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
            &excl("b", "E1", "hhs_oig", "CA", "2016-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        let sig = find(&r, "multi_state_exclusion").expect("multi_state fires");
        assert!(sig.confidence >= 0.70);
    }

    #[test]
    fn multi_state_true_negative_single_state() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(find(&r, "multi_state_exclusion").is_none());
    }

    // ── federal_state_mismatch (threshold boundary) ─────────────────────────

    #[test]
    fn fed_state_mismatch_at_threshold_is_queued() {
        // confidence is exactly 0.70 == threshold -> must classify as queued.
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        let sig = find(&r, "federal_state_mismatch").expect("fed_state fires");
        assert!((sig.confidence - 0.70).abs() < f64::EPSILON);
        assert_eq!(sig.reason_code, REASON_QUEUED_REVIEW);
    }

    #[test]
    fn fed_state_mismatch_suppressed_when_state_listed() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
            &excl("b", "E1", "state_medicaid", "HI", "2016-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(find(&r, "federal_state_mismatch").is_none());
    }

    #[test]
    fn fed_state_mismatch_disabled_without_jurisdiction() {
        let g = graph(&[
            &entity("E1", "HI"),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), None);
        assert!(find(&r, "federal_state_mismatch").is_none());
    }

    // ── active_npi_while_excluded ───────────────────────────────────────────

    #[test]
    fn active_npi_true_positive() {
        let g = graph(&[
            &format!(
                r#"{{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"E1","canonical_state":"HI","npis":["1234567890"],"npi_active":true}}"#
            ),
            &excl("a", "E1", "hhs_oig", "HI", "2021-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        let sig = find(&r, "active_npi_while_excluded").expect("active_npi fires");
        assert!(sig.confidence >= 0.85);
    }

    #[test]
    fn active_npi_true_negative_when_reinstated() {
        let g = graph(&[
            &format!(
                r#"{{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"E1","canonical_state":"HI","npis":["1234567890"],"npi_active":true}}"#
            ),
            &excl("a", "E1", "hhs_oig", "HI", "2015-01-01T00:00:00Z"),
            &reinst("b", "E1", "hhs_oig", "HI", "2018-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(find(&r, "active_npi_while_excluded").is_none());
    }

    // ── missing data / not computable / jurisdiction ────────────────────────

    #[test]
    fn entity_without_events_yields_no_signals() {
        let g = graph(&[&entity("E1", "HI")]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(r.signals.is_empty());
        assert_eq!(r.entities_evaluated, 1);
    }

    #[test]
    fn billing_after_exclusion_reported_not_computable() {
        let g = graph(&[&entity("E1", "HI")]);
        let r = compute(&g, &policy(), Some("HI"));
        assert!(r
            .not_computable
            .contains(&"billing_after_exclusion".to_string()));
    }

    #[test]
    fn jurisdiction_filter_excludes_other_states() {
        let g = graph(&[
            &entity("E1", "CA"),
            &excl("a", "E1", "hhs_oig", "CA", "2015-01-01T00:00:00Z"),
            &excl("b", "E1", "hhs_oig", "TX", "2016-01-01T00:00:00Z"),
        ]);
        let r = compute(&g, &policy(), Some("HI"));
        assert_eq!(r.entities_evaluated, 0);
        assert!(r.signals.is_empty());
    }
}
