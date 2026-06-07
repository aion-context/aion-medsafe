// SPDX-License-Identifier: MIT OR Apache-2.0
//! Provider Trust Graph — the verified input to signal computation.
//!
//! The pipeline (Python) resolves entities and lifecycle events, then emits a
//! typed-NDJSON `TrustGraphExport` (one tagged JSON object per line). That file
//! is sealed into a `.aion` whose payload IS the graph. This module verifies
//! that `.aion` (all four guarantees) before parsing a single record — no loose
//! NDJSON is ever trusted. See `pipeline/src/aion_medsafe_pipeline/schema.py`
//! for the producing contract.

use std::collections::HashMap;
use std::path::Path;

use aion_context::key_registry::KeyRegistry;
use serde::{Deserialize, Serialize};

use crate::error::{MedsafeError, Result};
use crate::provenance;

/// Upper bound on graph lines. National scope is ~hundreds of thousands of
/// records; this cap (Tiger Style: bound external input) guards against a
/// malformed or adversarial payload exhausting memory.
const MAX_GRAPH_LINES: usize = 5_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Individual,
    Organization,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionAuthority {
    HhsOig,
    StateMedicaid,
    SamGov,
    StateLicense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionStatus {
    Active,
    Reinstated,
    Indefinite,
}

/// Export-level metadata (first line of the NDJSON stream).
///
/// Retained in full for audit/display; the compute path does not read these
/// fields today. Mirrors `TrustGraphExport` metadata in schema.py.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct GraphMeta {
    pub export_version: String,
    pub exported_at: String,
    pub pipeline_version: String,
    #[serde(default)]
    pub entity_count: u64,
    #[serde(default)]
    pub exclusion_event_count: u64,
    #[serde(default)]
    pub sources_ingested: Vec<String>,
    #[serde(default)]
    pub jurisdiction_coverage: Vec<String>,
}

/// A resolved provider entity node.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Entity {
    pub entity_id: String,
    #[allow(dead_code)]
    pub entity_type: EntityType,
    pub canonical_name: String,
    #[serde(default)]
    pub canonical_state: Option<String>,
    #[serde(default)]
    pub npis: Vec<String>,
    /// Enrichment from NPPES: whether a matched NPI is currently active.
    /// `None` means no NPPES record was found (absence of data, not "inactive").
    #[serde(default)]
    pub npi_active: Option<bool>,
    /// RFC3339 deactivation date of a matched NPI (earliest), if any matched NPI
    /// is deactivated in NPPES. `None` = no deactivation on record.
    #[serde(default)]
    pub npi_deactivation_date: Option<String>,
    /// Normalized NPPES practice address (for co-location network detection).
    #[serde(default)]
    pub practice_address: Option<String>,
    /// Normalized NPPES practice phone (last 10 digits).
    #[serde(default)]
    pub practice_phone: Option<String>,
    /// Count of ACTIVE, non-excluded NPPES NPIs nationally sharing this entity's
    /// practice address — "is the clinic still operating under other identities?"
    #[serde(default)]
    pub addr_cohort_active: Option<u32>,
    /// Same, for the practice phone (a stronger same-operator signal).
    #[serde(default)]
    pub phone_cohort_active: Option<u32>,
    /// Active, non-excluded NPIs sharing BOTH this entity's address AND phone —
    /// the strongest "the same operation continues" signal.
    #[serde(default)]
    pub both_cohort_active: Option<u32>,
    /// Sample of those co-located active NPIs (evidence; capped).
    #[serde(default)]
    pub colocated_sample: Vec<String>,
    // resolution_confidence is carried from entity resolution for display/audit;
    // not yet read by the detectors.
    #[allow(dead_code)]
    #[serde(default = "default_confidence")]
    pub resolution_confidence: f64,
}

fn default_confidence() -> f64 {
    1.0
}

/// An exclusion or reinstatement event tied to an entity.
///
/// The source provenance fields (`source_id`, `source_record_id`,
/// `source_snapshot_hash`, `observed_at`) and `exclusion_type` are retained for
/// chain of custody and future detectors even though the current rules read
/// only dates, authority, state, and status. Mirrors schema.py.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct ExclusionEvent {
    pub event_id: String,
    pub entity_id: String,
    pub authority: ExclusionAuthority,
    #[serde(default)]
    pub exclusion_type: Option<String>,
    #[serde(default)]
    pub exclusion_date: Option<String>,
    #[serde(default)]
    pub reinstatement_date: Option<String>,
    pub status: ExclusionStatus,
    #[serde(default)]
    pub state: Option<String>,
    pub source_id: String,
    pub source_record_id: String,
    pub source_snapshot_hash: String,
    pub observed_at: String,
}

/// A suggested same-provider link below the auto-merge threshold — surfaced for
/// human review by entity resolution, never merged autonomously.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkCandidate {
    pub entity_a: String,
    pub entity_b: String,
    pub confidence: f64,
    #[serde(default)]
    pub signals: Vec<String>,
}

/// One line of the typed-NDJSON graph stream, dispatched on the `kind` tag.
///
/// `Other` makes the loader forward-compatible: any future kind is parsed and
/// skipped rather than failing the whole graph.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GraphLine {
    Meta(GraphMeta),
    Entity(Entity),
    ExclusionEvent(ExclusionEvent),
    IdentityLinkCandidate(LinkCandidate),
    #[serde(other)]
    Other,
}

/// The in-memory Trust Graph: entities plus their exclusion events, indexed by
/// `entity_id` for the detection engine.
#[derive(Debug, Clone, Default)]
pub struct TrustGraph {
    pub meta: Option<GraphMeta>,
    pub entities: Vec<Entity>,
    /// Sub-merge identity links surfaced for human review.
    pub link_candidates: Vec<LinkCandidate>,
    events_by_entity: HashMap<String, Vec<ExclusionEvent>>,
}

impl TrustGraph {
    /// Load a Trust Graph from a sealed `.aion`, verifying provenance first.
    ///
    /// REFUSES (returns `Err`) if the `.aion` fails any of the four guarantees.
    /// Only a fully verified payload is parsed.
    pub fn load_verified(aion_path: &Path, registry: &KeyRegistry) -> Result<Self> {
        let payload = provenance::load_verified_payload(aion_path, registry)?;
        let graph = Self::parse_ndjson(&payload, &aion_path.display().to_string())?;
        tracing::info!(
            event = "trust_graph_loaded",
            file = %aion_path.display(),
            entities = graph.entities.len(),
            entities_with_events = graph.events_by_entity.len(),
        );
        Ok(graph)
    }

    /// Parse typed-NDJSON bytes into a `TrustGraph`. Pure (no I/O) so it is
    /// directly testable with synthetic fixtures.
    pub fn parse_ndjson(bytes: &[u8], source_name: &str) -> Result<Self> {
        let mut graph = TrustGraph::default();

        for (idx, line) in bytes.split(|&b| b == b'\n').enumerate() {
            if idx >= MAX_GRAPH_LINES {
                return Err(MedsafeError::ParseError {
                    source_name: source_name.to_string(),
                    reason: format!("graph exceeds {MAX_GRAPH_LINES} lines"),
                });
            }
            // Skip blank lines (trailing newline, etc.)
            if line.iter().all(|b| b.is_ascii_whitespace()) {
                continue;
            }

            let parsed: GraphLine =
                serde_json::from_slice(line).map_err(|e| MedsafeError::ParseError {
                    source_name: source_name.to_string(),
                    reason: format!("line {}: {e}", idx + 1),
                })?;

            match parsed {
                GraphLine::Meta(m) => graph.meta = Some(m),
                GraphLine::Entity(e) => graph.entities.push(e),
                GraphLine::ExclusionEvent(ev) => {
                    graph
                        .events_by_entity
                        .entry(ev.entity_id.clone())
                        .or_default()
                        .push(ev);
                }
                GraphLine::IdentityLinkCandidate(c) => graph.link_candidates.push(c),
                // Forward-compatible: any other future kind is skipped.
                GraphLine::Other => {}
            }
        }

        Ok(graph)
    }

    /// Exclusion events recorded for an entity (empty slice if none).
    pub fn events_for(&self, entity_id: &str) -> &[ExclusionEvent] {
        self.events_by_entity
            .get(entity_id)
            .map_or(&[], |v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_context::crypto::SigningKey;
    use aion_context::types::AuthorId;

    const SAMPLE: &str = concat!(
        r#"{"kind":"meta","export_version":"1.0.0","exported_at":"t","pipeline_version":"0.1.0","entity_count":1,"exclusion_event_count":2}"#,
        "\n",
        r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE JANE","canonical_state":"HI","npis":["1234567890"],"npi_active":true}"#,
        "\n",
        r#"{"kind":"exclusion_event","event_id":"x1","entity_id":"E1","authority":"hhs_oig","exclusion_date":"2015-01-01T00:00:00Z","status":"active","state":"HI","source_id":"s","source_record_id":"r","source_snapshot_hash":"h","observed_at":"t"}"#,
        "\n",
        r#"{"kind":"exclusion_event","event_id":"x2","entity_id":"E1","authority":"state_medicaid","exclusion_date":"2016-01-01T00:00:00Z","status":"active","state":"HI","source_id":"s","source_record_id":"r","source_snapshot_hash":"h","observed_at":"t"}"#,
        "\n",
    );

    #[test]
    fn parses_meta_entities_events_and_indexes() {
        let g = TrustGraph::parse_ndjson(SAMPLE.as_bytes(), "test").expect("parse");
        assert_eq!(g.entities.len(), 1);
        assert_eq!(g.entities[0].entity_type, EntityType::Individual);
        assert_eq!(g.entities[0].npi_active, Some(true));
        assert_eq!(g.events_for("E1").len(), 2);
        assert_eq!(g.events_for("missing").len(), 0);
        assert!(g.meta.is_some());
    }

    #[test]
    fn parses_identity_link_candidates() {
        let line = r#"{"kind":"identity_link_candidate","entity_a":"E1","entity_b":"E2","confidence":0.83,"signals":["name_phonetic"]}"#;
        let g = TrustGraph::parse_ndjson(format!("{SAMPLE}{line}\n").as_bytes(), "test")
            .expect("parse");
        assert_eq!(g.link_candidates.len(), 1);
        assert_eq!(g.link_candidates[0].entity_a, "E1");
        assert_eq!(g.link_candidates[0].confidence, 0.83);
    }

    #[test]
    fn unknown_kind_is_ignored_not_errored() {
        let line = r#"{"kind":"some_future_kind","foo":1}"#;
        let g = TrustGraph::parse_ndjson(format!("{SAMPLE}{line}\n").as_bytes(), "test")
            .expect("parse");
        assert_eq!(g.entities.len(), 1);
        assert_eq!(g.link_candidates.len(), 0);
    }

    #[test]
    fn blank_lines_are_skipped() {
        let with_blanks = format!("\n{SAMPLE}\n  \n");
        let g = TrustGraph::parse_ndjson(with_blanks.as_bytes(), "test").expect("parse");
        assert_eq!(g.entities.len(), 1);
    }

    #[test]
    fn malformed_line_is_an_error_not_a_panic() {
        let bad = format!("{SAMPLE}{{not json}}\n");
        assert!(TrustGraph::parse_ndjson(bad.as_bytes(), "test").is_err());
    }

    fn registry_and_key() -> (KeyRegistry, SigningKey) {
        let key = SigningKey::generate();
        let mut registry = KeyRegistry::new();
        registry
            .register_author(
                AuthorId::new(80001),
                key.verifying_key(),
                key.verifying_key(),
                0,
            )
            .expect("register author");
        (registry, key)
    }

    #[test]
    fn load_verified_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("graph.aion");
        let (registry, key) = registry_and_key();
        crate::provenance::seal_payload(&path, SAMPLE.as_bytes(), &key, "seal graph")
            .expect("seal");

        let g = TrustGraph::load_verified(&path, &registry).expect("verified load");
        assert_eq!(g.entities.len(), 1);
    }

    #[test]
    fn tampered_graph_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("graph.aion");
        let (registry, key) = registry_and_key();
        crate::provenance::seal_payload(&path, SAMPLE.as_bytes(), &key, "seal graph")
            .expect("seal");

        // Flip a byte well past the magic header.
        let mut bytes = std::fs::read(&path).expect("read");
        let offset = bytes.len() / 2;
        bytes[offset] ^= 0x01;
        std::fs::write(&path, &bytes).expect("write tampered");

        // The verified loader must REFUSE — never return unverified graph data.
        assert!(TrustGraph::load_verified(&path, &registry).is_err());
    }
}
