// SPDX-License-Identifier: MIT OR Apache-2.0
//! Investigator case-packet generation — the court-defensible deliverable.
//!
//! For each flagged provider (an entity with at least one queued signal), a
//! packet assembles, in one place: the resolved identity, the risk signals, the
//! underlying exclusion evidence WITH its source provenance hashes, and an
//! attestation binding the packet to the verified graph + policy. The packets
//! are sealed into a `.aion` (their own chain of custody) and rendered to
//! human-readable Markdown for an investigator.
//!
//! Nothing here accuses — a packet is an evidence dossier for human review and,
//! if escalated, for an administrative hearing or court.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::detection::{self, ComputedSignal};
use crate::graph::{Entity, ExclusionAuthority, ExclusionEvent, TrustGraph};
use crate::owners::{ExcludedOwnerFinding, OwnedProvider};
use crate::policy::DetectionPolicy;
use crate::provenance;

const REVIEW: &str = "signal_queued_review";

/// Provenance attestation binding a packet to the verified inputs.
#[derive(Debug, Clone, Serialize)]
pub struct Attestation {
    pub graph_manifest_blake3: String,
    pub policy_version: String,
    pub policy_manifest_blake3: String,
    pub registry: String,
}

/// One piece of exclusion evidence with its source provenance.
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceItem {
    pub event_id: String,
    pub authority: String,
    pub exclusion_date: Option<String>,
    pub reinstatement_date: Option<String>,
    pub status: String,
    pub state: Option<String>,
    pub source_id: String,
    pub source_record_id: String,
    pub source_snapshot_blake3: String,
    /// Basis/cause of the exclusion (e.g. SAM exclusion type + excluding agency).
    #[serde(default)]
    pub basis: Option<String>,
}

impl EvidenceItem {
    fn from_event(e: &ExclusionEvent) -> Self {
        Self {
            event_id: e.event_id.clone(),
            authority: authority_label(&e.authority),
            exclusion_date: e.exclusion_date.clone(),
            reinstatement_date: e.reinstatement_date.clone(),
            status: format!("{:?}", e.status),
            state: e.state.clone(),
            source_id: e.source_id.clone(),
            source_record_id: e.source_record_id.clone(),
            source_snapshot_blake3: e.source_snapshot_hash.clone(),
            basis: e.exclusion_type.clone().filter(|s| !s.is_empty()),
        }
    }
}

/// A complete case packet for one flagged provider.
#[derive(Debug, Clone, Serialize)]
pub struct CasePacket {
    pub record: &'static str,
    pub packet_id: String,
    pub generated_at: String,
    pub jurisdiction: Option<String>,
    pub entity_id: String,
    pub canonical_name: String,
    pub canonical_state: Option<String>,
    pub npis: Vec<String>,
    pub npi_active: Option<bool>,
    pub resolution_confidence: f64,
    /// Which federal exclusion lists name this provider (HHS-OIG LEIE, SAM.gov).
    pub federal_lists: Vec<String>,
    /// Whether the provider is on the state Medicaid exclusion list.
    pub on_state_medicaid: bool,
    /// If this excluded party also OWNS active Medicare providers (CMS PECOS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership: Option<OwnershipFinding>,
    pub signals: Vec<ComputedSignal>,
    pub evidence: Vec<EvidenceItem>,
    pub attestation: Attestation,
}

/// Ownership of active Medicare providers by this excluded party (CMS PECOS),
/// folded in from the excluded-owner correlation.
#[derive(Debug, Clone, Serialize)]
pub struct OwnershipFinding {
    pub confidence: f64,
    pub state_corroborated: bool,
    pub owned_count: usize,
    pub owned_sample: Vec<OwnedProvider>,
}

impl From<&ExcludedOwnerFinding> for OwnershipFinding {
    fn from(f: &ExcludedOwnerFinding) -> Self {
        Self {
            confidence: f.confidence,
            state_corroborated: f.state_corroborated,
            owned_count: f.owned_count,
            owned_sample: f.owned_sample.clone(),
        }
    }
}

/// Summarize which exclusion lists name a provider, from its events.
fn coverage(events: &[ExclusionEvent]) -> (Vec<String>, bool) {
    use std::collections::BTreeSet;
    let mut federal: BTreeSet<String> = BTreeSet::new();
    let mut state_medicaid = false;
    for e in events {
        match e.authority {
            ExclusionAuthority::HhsOig => {
                federal.insert("HHS-OIG (LEIE)".to_string());
            }
            ExclusionAuthority::SamGov => {
                federal.insert("SAM.gov".to_string());
            }
            ExclusionAuthority::StateMedicaid => state_medicaid = true,
            ExclusionAuthority::StateLicense => {}
        }
    }
    (federal.into_iter().collect(), state_medicaid)
}

fn authority_label(a: &crate::graph::ExclusionAuthority) -> String {
    use crate::graph::ExclusionAuthority::*;
    match a {
        HhsOig => "HHS-OIG (federal LEIE)",
        StateMedicaid => "State Medicaid",
        SamGov => "SAM.gov (federal procurement)",
        StateLicense => "State licensing board",
    }
    .to_string()
}

fn packet_id(entity_id: &str, policy_version: &str, generated_at: &str) -> String {
    let digest = blake3::hash(format!("{entity_id}|{policy_version}|{generated_at}").as_bytes());
    hex::encode(&digest.as_bytes()[..16])
}

/// Build case packets for every flagged entity (pure — no I/O, directly
/// testable). A flagged entity has ≥1 queued signal. `entity_filter`, if set,
/// restricts to a single entity id. Deterministic (sorted by entity id).
#[allow(clippy::too_many_arguments)]
pub fn build_packets(
    graph: &TrustGraph,
    policy: &DetectionPolicy,
    jurisdiction: Option<&str>,
    attestation: &Attestation,
    generated_at: &str,
    entity_filter: Option<&str>,
    owner_findings: &BTreeMap<String, ExcludedOwnerFinding>,
) -> Vec<CasePacket> {
    let report = detection::compute(graph, policy, jurisdiction);
    let entity_by_id: HashMap<&str, &Entity> = graph
        .entities
        .iter()
        .map(|e| (e.entity_id.as_str(), e))
        .collect();

    let mut by_entity: BTreeMap<String, Vec<ComputedSignal>> = BTreeMap::new();
    for signal in report.signals {
        by_entity
            .entry(signal.entity_id.clone())
            .or_default()
            .push(signal);
    }

    let mut packets = Vec::new();
    for (entity_id, signals) in by_entity {
        if let Some(want) = entity_filter {
            if entity_id != want {
                continue;
            }
        }
        if !signals.iter().any(|s| s.reason_code == REVIEW) {
            continue; // only flagged (queued) providers get a packet
        }
        let Some(entity) = entity_by_id.get(entity_id.as_str()) else {
            continue;
        };
        let events = graph.events_for(&entity_id);
        let evidence = events.iter().map(EvidenceItem::from_event).collect();
        let (federal_lists, on_state_medicaid) = coverage(events);
        let ownership = owner_findings.get(&entity_id).map(OwnershipFinding::from);
        packets.push(CasePacket {
            record: "case_packet",
            packet_id: packet_id(&entity_id, &attestation.policy_version, generated_at),
            generated_at: generated_at.to_string(),
            jurisdiction: jurisdiction.map(str::to_string),
            entity_id: entity_id.clone(),
            canonical_name: entity.canonical_name.clone(),
            canonical_state: entity.canonical_state.clone(),
            npis: entity.npis.clone(),
            npi_active: entity.npi_active,
            resolution_confidence: entity.resolution_confidence,
            federal_lists,
            on_state_medicaid,
            ownership,
            signals,
            evidence,
            attestation: attestation.clone(),
        });
    }
    packets
}

/// Render a packet as a human-readable Markdown dossier.
pub fn render_markdown(p: &CasePacket) -> String {
    let mut m = String::new();
    m.push_str(&format!("# Case Packet {}\n\n", p.packet_id));
    m.push_str("> Investigative lead for human review — NOT an accusation or finding.\n\n");
    m.push_str(&format!("- Generated: {}\n", p.generated_at));
    if let Some(j) = &p.jurisdiction {
        m.push_str(&format!("- Jurisdiction: {j}\n"));
    }
    m.push_str("\n## Subject\n");
    m.push_str(&format!("- Name: **{}**\n", p.canonical_name));
    m.push_str(&format!("- Entity ID: `{}`\n", p.entity_id));
    if let Some(s) = &p.canonical_state {
        m.push_str(&format!("- State: {s}\n"));
    }
    if !p.npis.is_empty() {
        m.push_str(&format!("- NPIs: {}\n", p.npis.join(", ")));
    }
    if let Some(active) = p.npi_active {
        m.push_str(&format!("- NPI active in NPPES: {active}\n"));
    }
    m.push_str(&format!(
        "- Entity-resolution confidence: {:.2}\n",
        p.resolution_confidence
    ));
    let federal = if p.federal_lists.is_empty() {
        "none".to_string()
    } else {
        p.federal_lists.join(", ")
    };
    m.push_str(&format!("- Federal exclusion lists: {federal}\n"));
    m.push_str(&format!(
        "- On state Medicaid exclusion list: {}\n",
        p.on_state_medicaid
    ));

    m.push_str("\n## Risk signals\n");
    for s in &p.signals {
        let earned = s
            .calibrated_precision
            .map(|p| format!(", historical precision {:.0}%", p * 100.0))
            .unwrap_or_default();
        m.push_str(&format!(
            "- **{}** [confidence {:.2}, severity {:.2}{}] — {} _({}; review: {})_\n",
            s.signal_type,
            s.confidence,
            s.severity,
            earned,
            s.description,
            s.reason_code,
            s.requires_human_review
        ));
    }

    m.push_str("\n## Exclusion evidence (with source provenance)\n");
    for e in &p.evidence {
        let basis = e
            .basis
            .as_deref()
            .map(|b| format!(" | basis: {b}"))
            .unwrap_or_default();
        m.push_str(&format!(
            "- {} | excl {} | reinst {} | status {} | state {}{} | source `{}` rec `{}` | snapshot BLAKE3 `{}`\n",
            e.authority,
            e.exclusion_date.as_deref().unwrap_or("—"),
            e.reinstatement_date.as_deref().unwrap_or("—"),
            e.status,
            e.state.as_deref().unwrap_or("—"),
            basis,
            e.source_id,
            e.source_record_id,
            e.source_snapshot_blake3,
        ));
    }

    if let Some(o) = &p.ownership {
        let corr = if o.state_corroborated {
            "state-corroborated"
        } else {
            "name-only — verify manually"
        };
        m.push_str("\n## Ownership of active Medicare providers (CMS PECOS)\n");
        m.push_str(&format!(
            "- This excluded party matches an OWNER of {} active Medicare provider(s) [{:.2}, {}]\n",
            o.owned_count, o.confidence, corr
        ));
        for op in &o.owned_sample {
            let pct = op
                .ownership_pct
                .map(|v| format!(", {v}%"))
                .unwrap_or_default();
            let role = op.role.as_deref().unwrap_or("—");
            m.push_str(&format!(
                "  - {} ({}) — {role}{pct}\n",
                op.provider_org_name, op.provider_type
            ));
        }
    }

    m.push_str("\n## Provenance attestation\n");
    m.push_str(&format!(
        "- Trust Graph BLAKE3: `{}`\n",
        p.attestation.graph_manifest_blake3
    ));
    m.push_str(&format!(
        "- Detection policy: v{} (BLAKE3 `{}`)\n",
        p.attestation.policy_version, p.attestation.policy_manifest_blake3
    ));
    m.push_str(&format!("- Key registry: `{}`\n", p.attestation.registry));
    m.push_str(
        "\n_Verify: re-run `aion-medsafe provenance` on the graph/policy `.aion` and confirm \
         the hashes above against the registry. Every signal and evidence item is reproducible \
         from the sealed sources._\n",
    );
    m
}

/// Generate, seal, and render case packets for flagged providers.
#[allow(clippy::too_many_arguments)]
pub fn run(
    policy_path: &Path,
    graph_path: &Path,
    jurisdiction: Option<&str>,
    entity_filter: Option<&str>,
    output: Option<&Path>,
    render_dir: &Path,
    limit: Option<usize>,
    owners_path: Option<&Path>,
) -> anyhow::Result<()> {
    let registry_path = Path::new(provenance::DEFAULT_REGISTRY_PATH);
    let registry = provenance::load_registry(registry_path)?;

    // Load + verify both inputs; capture their content hashes for attestation.
    let graph_payload = provenance::load_verified_payload(graph_path, &registry)?;
    let graph_hash = hex::encode(blake3::hash(&graph_payload).as_bytes());
    let graph = TrustGraph::parse_ndjson(&graph_payload, &graph_path.display().to_string())?;

    let policy_payload = provenance::load_verified_payload(policy_path, &registry)?;
    let policy_hash = hex::encode(blake3::hash(&policy_payload).as_bytes());
    let policy: DetectionPolicy = serde_yaml::from_slice(&policy_payload).map_err(|e| {
        crate::error::MedsafeError::ParseError {
            source_name: policy_path.display().to_string(),
            reason: e.to_string(),
        }
    })?;

    let attestation = Attestation {
        graph_manifest_blake3: graph_hash,
        policy_version: policy.version.clone(),
        policy_manifest_blake3: policy_hash,
        registry: registry_path.display().to_string(),
    };
    let generated_at = Utc::now().to_rfc3339();

    // Optionally fold in the excluded-owner correlation (CMS PECOS ownership).
    let owner_findings = match owners_path {
        Some(p) => crate::owners::findings_by_entity(&graph, p, jurisdiction)?,
        None => BTreeMap::new(),
    };

    let mut packets = build_packets(
        &graph,
        &policy,
        jurisdiction,
        &attestation,
        &generated_at,
        entity_filter,
        &owner_findings,
    );
    let total = packets.len();
    if let Some(n) = limit {
        packets.truncate(n);
    }

    // Annotate each packet's signals with earned precision (calibration loop).
    let calibration = crate::adjudication::calibrate(
        &crate::adjudication::load(
            Path::new(crate::adjudication::DEFAULT_ADJUDICATIONS_PATH),
            &registry,
        )
        .unwrap_or_default(),
    );
    for p in &mut packets {
        crate::adjudication::annotate(&mut p.signals, &calibration);
    }

    // Seal the packets (their own chain of custody).
    let mut payload = String::new();
    payload.push_str(
        &serde_json::json!({
            "record": "packet_run_meta",
            "generated_at": generated_at,
            "jurisdiction": jurisdiction.unwrap_or("national"),
            "flagged_providers": total,
            "packets_written": packets.len(),
        })
        .to_string(),
    );
    payload.push('\n');
    for p in &packets {
        if let Ok(line) = serde_json::to_string(p) {
            payload.push_str(&line);
            payload.push('\n');
        }
    }
    let out_path = output.map(Path::to_path_buf).unwrap_or_else(|| {
        let jur = jurisdiction.unwrap_or("national").to_lowercase();
        let stamp = Utc::now().format("%Y-%m-%dT%H%M%S%.3fZ");
        Path::new("provenance").join(format!("case_packets_{jur}_{stamp}.aion"))
    });
    let signing_key = provenance::load_signing_key()?;
    let sealed = provenance::seal_payload(
        &out_path,
        payload.as_bytes(),
        &signing_key,
        &format!("Case packets {}", jurisdiction.unwrap_or("national")),
    )?;

    // Render Markdown dossiers.
    std::fs::create_dir_all(render_dir)?;
    for p in &packets {
        let path = render_dir.join(format!("{}.md", p.packet_id));
        std::fs::write(path, render_markdown(p))?;
    }

    println!("✓ Case packets generated");
    println!("  Flagged providers: {total}");
    println!("  Packets written: {}", packets.len());
    println!(
        "  Sealed: {} (BLAKE3 {})",
        out_path.display(),
        hex::encode(sealed)
    );
    println!("  Markdown: {}/", render_dir.display());
    if let Some(sample) = packets.first() {
        println!("\n──────── sample packet ────────\n");
        println!("{}", render_markdown(sample));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_YAML: &str = r#"
version: "1"
effective_date: "2026-06-06"
thresholds: {minimum_confidence_for_alert: 0.70, maximum_days_lookback: 3650}
jurisdictions: {primary: "HI", scope: "test"}
risk_signals:
  multi_state_exclusion: {severity: 0.70, description: "x", requires_human_review: true}
"#;

    fn attestation() -> Attestation {
        Attestation {
            graph_manifest_blake3: "ghash".into(),
            policy_version: "1".into(),
            policy_manifest_blake3: "phash".into(),
            registry: ".aion/medsafe.registry.json".into(),
        }
    }

    fn no_owners() -> BTreeMap<String, ExcludedOwnerFinding> {
        BTreeMap::new()
    }

    #[test]
    fn packet_assembles_identity_signals_evidence_and_attestation() {
        let lines = [
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE JANE","canonical_state":"HI","npis":["1234567890"]}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"a","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2015-01-01T00:00:00Z","status":"active","state":"HI","source_id":"hhs_oig_leie","source_record_id":"7","source_snapshot_hash":"abc123","observed_at":"t"}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"b","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2016-01-01T00:00:00Z","status":"active","state":"CA","source_id":"hhs_oig_leie","source_record_id":"8","source_snapshot_hash":"def456","observed_at":"t"}"#.to_string(),
        ];
        let graph = TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "t").unwrap();
        let policy: DetectionPolicy = serde_yaml::from_str(POLICY_YAML).unwrap();

        let packets = build_packets(
            &graph,
            &policy,
            Some("HI"),
            &attestation(),
            "2026-06-07T00:00:00Z",
            None,
            &no_owners(),
        );
        assert_eq!(packets.len(), 1);
        let p = &packets[0];
        assert_eq!(p.entity_id, "E1");
        assert!(p
            .signals
            .iter()
            .any(|s| s.signal_type == "multi_state_exclusion"));
        assert_eq!(p.evidence.len(), 2);
        assert!(p
            .evidence
            .iter()
            .any(|e| e.source_snapshot_blake3 == "abc123"));
        assert_eq!(p.attestation.graph_manifest_blake3, "ghash");
        // Markdown renders the provenance section.
        let md = render_markdown(p);
        assert!(md.contains("Provenance attestation"));
        assert!(md.contains("abc123"));
    }

    #[test]
    fn sam_event_appears_in_federal_coverage_and_evidence_basis() {
        // HHS-OIG (HI) + SAM.gov (CA, with basis) -> multi_state flags it; the
        // packet must show both federal lists and the SAM exclusion basis.
        let lines = [
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE JANE","canonical_state":"HI","npis":["1234567890"]}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"a","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2015-01-01T00:00:00Z","status":"active","state":"HI","source_id":"hhs_oig_leie","source_record_id":"7","source_snapshot_hash":"abc123","observed_at":"t"}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"b","entity_id":"E1","authority":"sam_gov","exclusion_type":"Healthcare Fraud — HHS/OIG","exclusion_date":"2016-01-01T00:00:00Z","status":"indefinite","state":"CA","source_id":"sam_gov","source_record_id":"S4M1","source_snapshot_hash":"sam999","observed_at":"t"}"#.to_string(),
        ];
        let graph = TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "t").unwrap();
        let policy: DetectionPolicy = serde_yaml::from_str(POLICY_YAML).unwrap();
        let packets = build_packets(
            &graph,
            &policy,
            Some("HI"),
            &attestation(),
            "t",
            None,
            &no_owners(),
        );
        assert_eq!(packets.len(), 1);
        let p = &packets[0];
        assert_eq!(p.federal_lists, vec!["HHS-OIG (LEIE)", "SAM.gov"]);
        assert!(!p.on_state_medicaid);
        let sam = p
            .evidence
            .iter()
            .find(|e| e.source_id == "sam_gov")
            .expect("SAM evidence present");
        assert_eq!(sam.basis.as_deref(), Some("Healthcare Fraud — HHS/OIG"));
        let md = render_markdown(p);
        assert!(md.contains("Federal exclusion lists: HHS-OIG (LEIE), SAM.gov"));
        assert!(md.contains("basis: Healthcare Fraud"));
    }

    #[test]
    fn excluded_owner_finding_is_folded_into_packet() {
        let lines = [
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE JANE","canonical_state":"HI"}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"a","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2015-01-01T00:00:00Z","status":"active","state":"HI","source_id":"hhs_oig_leie","source_record_id":"7","source_snapshot_hash":"abc","observed_at":"t"}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"b","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2016-01-01T00:00:00Z","status":"active","state":"CA","source_id":"hhs_oig_leie","source_record_id":"8","source_snapshot_hash":"def","observed_at":"t"}"#.to_string(),
        ];
        let graph = TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "t").unwrap();
        let policy: DetectionPolicy = serde_yaml::from_str(POLICY_YAML).unwrap();
        let mut owners = BTreeMap::new();
        owners.insert(
            "E1".to_string(),
            ExcludedOwnerFinding {
                record: "excluded_owner",
                entity_id: "E1".into(),
                entity_name: "DOE JANE".into(),
                entity_state: Some("HI".into()),
                confidence: 0.85,
                reason_code: "signal_queued_review",
                state_corroborated: true,
                owned_count: 1,
                owned_sample: vec![OwnedProvider {
                    provider_org_name: "SUNSET SNF".into(),
                    provider_type: "SNF".into(),
                    role: Some("5% OR GREATER DIRECT OWNERSHIP INTEREST".into()),
                    ownership_pct: Some(50.0),
                }],
            },
        );
        let packets = build_packets(
            &graph,
            &policy,
            Some("HI"),
            &attestation(),
            "t",
            None,
            &owners,
        );
        let p = &packets[0];
        let owned = p.ownership.as_ref().expect("ownership folded in");
        assert_eq!(owned.owned_count, 1);
        assert!(owned.state_corroborated);
        let md = render_markdown(p);
        assert!(md.contains("Ownership of active Medicare providers"));
        assert!(md.contains("SUNSET SNF"));
    }

    #[test]
    fn unflagged_entity_gets_no_packet() {
        // A single-state exclusion -> no multi_state signal -> not flagged.
        let lines = [
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"SOLO","canonical_state":"HI"}"#.to_string(),
            r#"{"kind":"exclusion_event","event_id":"a","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2015-01-01T00:00:00Z","status":"active","state":"HI","source_id":"s","source_record_id":"1","source_snapshot_hash":"h","observed_at":"t"}"#.to_string(),
        ];
        let graph = TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "t").unwrap();
        let policy: DetectionPolicy = serde_yaml::from_str(POLICY_YAML).unwrap();
        let packets = build_packets(
            &graph,
            &policy,
            Some("HI"),
            &attestation(),
            "t",
            None,
            &no_owners(),
        );
        assert!(packets.is_empty());
    }
}
