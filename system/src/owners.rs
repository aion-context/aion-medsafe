// SPDX-License-Identifier: MIT OR Apache-2.0
//! Excluded-owner correlation — is an EXCLUDED party an owner of active
//! Medicare providers?
//!
//! CMS PECOS "All Owners" data lists who owns each Medicare-enrolled provider
//! (SNF/HHA/Hospice/Hospital/FQHC/RHC). We match those owners against the
//! excluded universe in the sealed Trust Graph BY NAME (owners carry no NPI),
//! corroborated by state where available. A hit means an excluded individual or
//! entity is profiting from Medicare through ownership — a novel, high-value
//! lead. Like every signal here, it is evidence for human review, not an
//! accusation; findings are sealed into their own `.aion`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::{EntityType, TrustGraph};
use crate::provenance;
use crate::resolve::normalize_name;

/// Cap to keep an accidental/garbage owners file from exhausting memory.
const MAX_OWNER_ROWS: usize = 20_000_000;
const EVIDENCE_CAP: usize = 12;

#[derive(Debug, Clone, Deserialize)]
struct OwnerRow {
    #[serde(default)]
    owner_name: String,
    #[serde(default)]
    owner_type: Option<String>,
    #[serde(default)]
    owner_state: Option<String>,
    #[serde(default)]
    provider_org_name: Option<String>,
    #[serde(default)]
    provider_type: Option<String>,
    #[serde(default)]
    owner_role: Option<String>,
    #[serde(default)]
    ownership_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnedProvider {
    pub provider_org_name: String,
    pub provider_type: String,
    pub role: Option<String>,
    pub ownership_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExcludedOwnerFinding {
    pub record: &'static str,
    pub entity_id: String,
    pub entity_name: String,
    pub entity_state: Option<String>,
    pub confidence: f64,
    pub reason_code: &'static str,
    pub state_corroborated: bool,
    pub owned_count: usize,
    pub owned_sample: Vec<OwnedProvider>,
}

/// Result of the correlation: confirmed findings plus a count of suppressed
/// individual name-only candidates (reported for transparency — no silent caps).
#[derive(Debug, Default)]
pub struct Correlation {
    pub findings: Vec<ExcludedOwnerFinding>,
    pub suppressed_name_only: usize,
}

/// Index owner rows by normalized owner name (pure — testable). The role is kept
/// as evidence, not used as a filter: an excluded party in ANY ownership/control
/// position (owner, officer, director, managing control) at a Medicare provider
/// is a concern.
fn index_owners(rows: &[OwnerRow]) -> BTreeMap<String, Vec<&OwnerRow>> {
    let mut idx: BTreeMap<String, Vec<&OwnerRow>> = BTreeMap::new();
    for row in rows {
        let key = normalize_name(&row.owner_name);
        if !key.is_empty() {
            idx.entry(key).or_default().push(row);
        }
    }
    idx
}

/// Match excluded entities against the owner index (pure — testable).
///
/// Precision controls: a known owner state that DIFFERS from the excluded
/// entity's state rules the match out (different person/place). Name match with
/// state corroboration scores higher than a name-only match (which, for common
/// names, can be coincidental — hence human review).
fn correlate(graph: &TrustGraph, rows: &[OwnerRow], jurisdiction: Option<&str>) -> Correlation {
    let idx = index_owners(rows);
    let jur = jurisdiction.map(str::to_uppercase);
    let mut out = Correlation::default();

    for entity in &graph.entities {
        let estate = entity.canonical_state.as_deref().map(str::to_uppercase);
        if let (Some(j), Some(s)) = (&jur, &estate) {
            if j != s {
                continue;
            }
        } else if jur.is_some() && estate.is_none() {
            continue;
        }

        let key = normalize_name(&entity.canonical_name);
        if key.is_empty() {
            continue;
        }
        let Some(matches) = idx.get(&key) else {
            continue;
        };

        // The owner record's type must be compatible with the excluded entity
        // (individual<->'I', organization<->'O'); unknown entity type allows both.
        let want_otype = match entity.entity_type {
            EntityType::Individual => Some("I"),
            EntityType::Organization => Some("O"),
            EntityType::Unknown => None,
        };
        // Keep owner rows whose state agrees (or is unknown) and type is compatible.
        let kept: Vec<&&OwnerRow> = matches
            .iter()
            .filter(|r| {
                let state_ok = match (r.owner_state.as_deref().map(str::to_uppercase), &estate) {
                    (Some(os), Some(es)) => os == *es,
                    _ => true, // unknown owner state -> can't refute; keep (name-only)
                };
                let type_ok = match (want_otype, r.owner_type.as_deref()) {
                    (Some(w), Some(ot)) => ot.eq_ignore_ascii_case(w),
                    _ => true,
                };
                state_ok && type_ok
            })
            .collect();
        if kept.is_empty() {
            continue;
        }

        let state_corroborated = kept
            .iter()
            .any(|r| r.owner_state.as_deref().map(str::to_uppercase) == estate && estate.is_some());
        let is_org = matches!(entity.entity_type, EntityType::Organization);

        // Individual name-only matches are coincidence-prone (CMS owners carry no
        // NPI/DOB, and 0% of individual rows carry state). We keep ONLY the
        // credible slice: a DISTINCTIVE name (>=3 tokens, i.e. a middle name/
        // initial) owning a SMALL number of providers (<=2) — common-name
        // collisions surface as high owned counts and are suppressed (counted).
        // Organizations (distinctive names) and any state-corroborated match are
        // always kept.
        let name_tokens = key.split(' ').filter(|t| !t.is_empty()).count();
        let distinctive_low_count = name_tokens >= 3 && kept.len() <= 2;
        if !is_org && !state_corroborated && !distinctive_low_count {
            out.suppressed_name_only += 1;
            continue;
        }
        let confidence = if state_corroborated {
            0.85
        } else if is_org {
            0.75
        } else {
            0.55 // distinctive individual, name-only -> lead, verify manually
        };

        let owned_sample: Vec<OwnedProvider> = kept
            .iter()
            .take(EVIDENCE_CAP)
            .map(|r| OwnedProvider {
                provider_org_name: r.provider_org_name.clone().unwrap_or_default(),
                provider_type: r.provider_type.clone().unwrap_or_default(),
                role: r.owner_role.clone(),
                ownership_pct: r.ownership_pct,
            })
            .collect();

        out.findings.push(ExcludedOwnerFinding {
            record: "excluded_owner",
            entity_id: entity.entity_id.clone(),
            entity_name: entity.canonical_name.clone(),
            entity_state: entity.canonical_state.clone(),
            confidence,
            reason_code: "signal_queued_review",
            state_corroborated,
            owned_count: kept.len(),
            owned_sample,
        });
    }
    out.findings.sort_by(|a, b| {
        b.confidence
            .total_cmp(&a.confidence)
            .then(b.owned_count.cmp(&a.owned_count))
    });
    out
}

fn load_owners(path: &Path) -> anyhow::Result<Vec<OwnerRow>> {
    use std::io::{BufRead, BufReader};
    let reader = BufReader::new(std::fs::File::open(path)?);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if rows.len() >= MAX_OWNER_ROWS {
            anyhow::bail!("owners file exceeds {MAX_OWNER_ROWS} rows; refusing to load");
        }
        if let Ok(row) = serde_json::from_str::<OwnerRow>(&line) {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Verify the graph, load owners, correlate, seal the findings, print a summary.
pub fn run(
    graph_path: &Path,
    owners_path: &Path,
    jurisdiction: Option<&str>,
    output: Option<&Path>,
) -> anyhow::Result<()> {
    let registry = provenance::load_registry(Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let graph = TrustGraph::load_verified(graph_path, &registry)?;
    let rows = load_owners(owners_path)?;
    let result = correlate(&graph, &rows, jurisdiction);
    let findings = &result.findings;

    let mut payload = String::new();
    payload.push_str(
        &serde_json::json!({
            "record": "owners_run_meta",
            "jurisdiction": jurisdiction.unwrap_or("national"),
            "owner_rows": rows.len(),
            "excluded_owners_found": findings.len(),
            "suppressed_individual_name_only": result.suppressed_name_only,
        })
        .to_string(),
    );
    payload.push('\n');
    for f in findings {
        if let Ok(line) = serde_json::to_string(f) {
            payload.push_str(&line);
            payload.push('\n');
        }
    }
    let out_path = output.map(Path::to_path_buf).unwrap_or_else(|| {
        let jur = jurisdiction.unwrap_or("national").to_lowercase();
        let stamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%S%.3fZ");
        Path::new("provenance").join(format!("excluded_owners_{jur}_{stamp}.aion"))
    });
    let signing_key = provenance::load_signing_key()?;
    let sealed = provenance::seal_payload(
        &out_path,
        payload.as_bytes(),
        &signing_key,
        "Excluded-owner correlation",
    )?;

    println!("✓ Excluded-owner correlation (graph verified)");
    println!("  Owner rows scanned: {}", rows.len());
    println!("  Excluded parties found as owners: {}", findings.len());
    println!(
        "  Individual name-only matches suppressed (unreliable, no NPI/DOB): {}",
        result.suppressed_name_only
    );
    println!("  Sealed: {} ({})", out_path.display(), hex::encode(sealed));
    for f in findings.iter().take(10) {
        let corr = if f.state_corroborated {
            "state✓"
        } else {
            "name-only"
        };
        println!(
            "    [{:.2} {}] {} ({}) owns {} provider(s)",
            f.confidence,
            corr,
            f.entity_name,
            f.entity_state.as_deref().unwrap_or("?"),
            f.owned_count
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(name: &str, otype: &str, state: Option<&str>, provider: &str) -> OwnerRow {
        OwnerRow {
            owner_name: name.to_string(),
            owner_type: Some(otype.to_string()),
            owner_state: state.map(str::to_string),
            provider_org_name: Some(provider.to_string()),
            provider_type: Some("SNF".to_string()),
            owner_role: Some("5% OR MORE OWNERSHIP".to_string()),
            ownership_pct: Some(50.0),
        }
    }

    fn graph(lines: &[&str]) -> TrustGraph {
        TrustGraph::parse_ndjson(lines.join("\n").as_bytes(), "t").unwrap()
    }

    #[test]
    fn matches_excluded_individual_with_state_corroboration() {
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE, JANE","canonical_state":"HI"}"#,
        ]);
        let rows = vec![
            owner("DOE, JANE", "I", Some("HI"), "SUNSET SNF"),
            owner("DOE, JANE", "I", Some("HI"), "OCEAN SNF"),
        ];
        let f = correlate(&g, &rows, None).findings;
        assert_eq!(f.len(), 1);
        assert!(f[0].confidence >= 0.85);
        assert!(f[0].state_corroborated);
        assert_eq!(f[0].owned_count, 2);
    }

    #[test]
    fn differing_state_rules_out_match() {
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DOE, JANE","canonical_state":"HI"}"#,
        ]);
        let rows = vec![owner("DOE, JANE", "I", Some("CA"), "MAINLAND SNF")];
        assert!(correlate(&g, &rows, None).findings.is_empty());
    }

    #[test]
    fn name_only_individual_is_suppressed() {
        // No NPI/DOB/state for individual owners -> name-only is coincidence-prone,
        // so it is suppressed (counted, not emitted).
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"SMITH, JOHN","canonical_state":"HI"}"#,
        ]);
        let rows = vec![owner("SMITH, JOHN", "I", None, "SOME SNF")];
        let r = correlate(&g, &rows, None);
        assert!(r.findings.is_empty());
        assert_eq!(r.suppressed_name_only, 1);
    }

    #[test]
    fn distinctive_individual_low_count_is_kept_as_lead() {
        // 3-token name owning a single provider -> credible lead at 0.55.
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"ARRINGTON, CLIFTON W","canonical_state":"HI"}"#,
        ]);
        let rows = vec![owner("ARRINGTON, CLIFTON W", "I", None, "ONE SNF")];
        let r = correlate(&g, &rows, None);
        assert_eq!(r.findings.len(), 1);
        assert!((r.findings[0].confidence - 0.55).abs() < 1e-9);
        assert_eq!(r.suppressed_name_only, 0);
    }

    #[test]
    fn distinctive_individual_high_count_is_suppressed_as_collision() {
        // Distinctive name but owning many providers -> collision-prone, suppressed.
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"DAVIS, DAVID W","canonical_state":"HI"}"#,
        ]);
        let rows = vec![
            owner("DAVIS, DAVID W", "I", None, "A SNF"),
            owner("DAVIS, DAVID W", "I", None, "B SNF"),
            owner("DAVIS, DAVID W", "I", None, "C SNF"),
        ];
        let r = correlate(&g, &rows, None);
        assert!(r.findings.is_empty());
        assert_eq!(r.suppressed_name_only, 1);
    }

    #[test]
    fn org_owner_matches_by_name() {
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"organization","canonical_name":"ACME HOLDINGS LLC","canonical_state":"HI"}"#,
        ]);
        let rows = vec![owner("ACME HOLDINGS LLC", "O", None, "SUNSET SNF")];
        let f = correlate(&g, &rows, None).findings;
        assert_eq!(f.len(), 1);
        assert!((f[0].confidence - 0.75).abs() < 1e-9);
    }

    #[test]
    fn no_match_when_name_absent_from_owners() {
        let g = graph(&[
            r#"{"kind":"entity","entity_id":"E1","entity_type":"individual","canonical_name":"NOBODY, NEMO","canonical_state":"HI"}"#,
        ]);
        let rows = vec![owner("DOE, JANE", "I", Some("HI"), "X")];
        assert!(correlate(&g, &rows, None).findings.is_empty());
    }
}
