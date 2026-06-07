// SPDX-License-Identifier: MIT OR Apache-2.0
//! Build the Trust Graph from normalized pipeline output — Rust-owned compute.
//!
//! Python's job ends at normalization; this module reads the per-source
//! normalized NDJSON, resolves entities (resolve.rs), reconstructs
//! exclusion/reinstatement events across federal and state sources, enriches
//! with NPPES NPI status, and seals the typed-NDJSON Trust Graph into a `.aion`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decisions;
use crate::graph::{Entity, EntityType, ExclusionAuthority, ExclusionEvent, ExclusionStatus};
use crate::provenance;
use crate::resolve::{self, ResolveInput};

const EXCLUSION_FILES: &[&str] = &[
    "leie_normalized.ndjson",
    "leie_supplements_normalized.ndjson",
    "hawaii_medquest_exclusions.ndjson",
];

const ORG_TOKENS: &[&str] = &[
    "INC",
    "LLC",
    "LLP",
    "CORP",
    "CO",
    "COMPANY",
    "CENTER",
    "CENTERS",
    "SERVICE",
    "SERVICES",
    "HOSPITAL",
    "CLINIC",
    "PHARMACY",
    "LAB",
    "LABS",
    "LABORATORY",
    "GROUP",
    "ASSOC",
    "ASSOCIATES",
    "HEALTH",
    "HOME",
    "AGENCY",
    "FOUNDATION",
    "SYSTEMS",
    "SOLUTIONS",
    "ENTERPRISES",
    "MARKETING",
];

#[derive(Debug, Default, Deserialize)]
struct NormalizedExclusion {
    #[serde(default)]
    person_or_entity_name: String,
    #[serde(default)]
    npi: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    exclusion_date: Option<String>,
    #[serde(default)]
    reinstatement_date: Option<String>,
    #[serde(default)]
    source_id: String,
    #[serde(default)]
    source_record_id: Option<String>,
    #[serde(default)]
    medicaid_provider_id: Option<String>,
    #[serde(default)]
    provider_type: Option<String>,
    #[serde(default)]
    indefinite_exclusion: bool,
    #[serde(default)]
    source_snapshot_hash: String,
    #[serde(default)]
    observed_at: Option<String>,
}

impl NormalizedExclusion {
    fn npi_norm(&self) -> Option<String> {
        self.npi
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    }

    fn record_id(&self) -> String {
        self.source_record_id
            .clone()
            .or_else(|| self.medicaid_provider_id.clone())
            .unwrap_or_default()
    }
}

#[derive(Serialize)]
struct LinkCandidateLine {
    entity_a: String,
    entity_b: String,
    confidence: f64,
    signals: Vec<String>,
}

#[derive(Debug, Default)]
pub struct BuildStats {
    pub records: usize,
    pub entities: usize,
    pub events: usize,
    pub candidates: usize,
    pub npi_merges: usize,
    pub name_merges: usize,
    pub fuzzy_auto_links: usize,
    pub blocks_capped: usize,
    pub confirmed_applied: usize,
    pub rejected: usize,
}

/// Read normalized NDJSON, resolve, apply human review decisions, build the
/// typed graph, and seal it.
pub fn run(
    normalized_dir: &Path,
    output: &Path,
    decisions_path: &Path,
) -> anyhow::Result<BuildStats> {
    let records = load_records(normalized_dir)?;
    if records.is_empty() {
        anyhow::bail!(
            "no normalized exclusion records found in {}",
            normalized_dir.display()
        );
    }
    let npis: Vec<Option<String>> = records.iter().map(NormalizedExclusion::npi_norm).collect();
    // Only the excluded providers' NPIs are needed for enrichment — filter the
    // national NPPES table (millions of rows) down to those on load.
    let wanted: BTreeSet<&str> = npis.iter().filter_map(|n| n.as_deref()).collect();
    let nppes = load_nppes(&normalized_dir.join("nppes_providers.ndjson"), &wanted)?;
    let inputs: Vec<ResolveInput> = records
        .iter()
        .enumerate()
        .map(|(i, r)| ResolveInput {
            name: &r.person_or_entity_name,
            npi: npis[i].as_deref(),
            state: r.state.as_deref(),
        })
        .collect();

    let resolved = resolve::resolve(&inputs);

    // Apply human review decisions (verified): confirm → force-merge clusters;
    // reject → suppress from the emitted candidate queue.
    let registry = provenance::load_registry(Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let verdicts = decisions::verdicts(&decisions::load(decisions_path, &registry)?);

    let mut cluster_of = resolved.cluster_of.clone();
    let clusters0 = group_clusters(&cluster_of);
    let ids0 = entity_ids(&clusters0, &records, &npis);
    let confirmed_applied = apply_confirmed(&mut cluster_of, &ids0, &verdicts.confirmed);

    let clusters = group_clusters(&cluster_of);
    let entity_id_by_rep = entity_ids(&clusters, &records, &npis);

    let payload = render(
        &records,
        &npis,
        &nppes,
        &clusters,
        &entity_id_by_rep,
        &resolved,
        &cluster_of,
        &verdicts.rejected,
    );

    let signing_key = provenance::load_signing_key()?;
    provenance::seal_payload(
        output,
        payload.as_bytes(),
        &signing_key,
        "Build trust graph",
    )?;

    Ok(BuildStats {
        records: records.len(),
        entities: clusters.len(),
        events: payload.matches("\"kind\":\"exclusion_event\"").count(),
        candidates: payload
            .matches("\"kind\":\"identity_link_candidate\"")
            .count(),
        npi_merges: resolved.stats.npi_merges,
        name_merges: resolved.stats.name_merges,
        fuzzy_auto_links: resolved.stats.fuzzy_auto_links,
        blocks_capped: resolved.stats.blocks_capped,
        confirmed_applied,
        rejected: verdicts.rejected.len(),
    })
}

fn load_records(dir: &Path) -> anyhow::Result<Vec<NormalizedExclusion>> {
    let mut records = Vec::new();
    for filename in EXCLUSION_FILES {
        let path = dir.join(filename);
        if !path.exists() {
            continue;
        }
        let reader = BufReader::new(std::fs::File::open(&path)?);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let rec: NormalizedExclusion = serde_json::from_str(&line).map_err(|e| {
                crate::error::MedsafeError::ParseError {
                    source_name: path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            if !rec.person_or_entity_name.trim().is_empty() {
                records.push(rec);
            }
        }
    }
    Ok(records)
}

/// NPPES status for one NPI: active flag + (raw) deactivation date if any.
struct NppesInfo {
    active: bool,
    deactivation_date: Option<String>,
}

/// Map NPI -> status info, from the normalized NPPES bulk table, keeping only
/// NPIs in `wanted` (the excluded providers). The national table is millions of
/// rows; we stream it but retain only the relevant subset.
/// Produced by `aion-medsafe-pipeline nppes-bulk`.
fn load_nppes(path: &Path, wanted: &BTreeSet<&str>) -> anyhow::Result<BTreeMap<String, NppesInfo>> {
    #[derive(Deserialize)]
    struct NppesRow {
        #[serde(default)]
        npi: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        deactivation_date: Option<String>,
    }

    let mut map = BTreeMap::new();
    if !path.exists() {
        return Ok(map);
    }
    let reader = BufReader::new(std::fs::File::open(path)?);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: NppesRow = serde_json::from_str(&line)?;
        if !row.npi.is_empty() && wanted.contains(row.npi.as_str()) {
            map.insert(
                row.npi,
                NppesInfo {
                    active: row.status == "ACTIVE",
                    deactivation_date: row.deactivation_date,
                },
            );
        }
    }
    Ok(map)
}

#[allow(clippy::too_many_arguments)]
fn render(
    records: &[NormalizedExclusion],
    npis: &[Option<String>],
    nppes: &BTreeMap<String, NppesInfo>,
    clusters: &BTreeMap<usize, Vec<usize>>,
    entity_id_by_rep: &BTreeMap<usize, String>,
    resolved: &resolve::ResolveResult,
    cluster_of: &[usize],
    rejected: &BTreeSet<(String, String)>,
) -> String {
    let mut entities = Vec::with_capacity(clusters.len());
    let mut coverage: BTreeSet<String> = BTreeSet::new();
    for (rep, members) in clusters {
        let entity = build_entity(entity_id_by_rep[rep].clone(), members, records, npis, nppes);
        if let Some(state) = &entity.canonical_state {
            coverage.insert(state.clone());
        }
        entities.push(entity);
    }

    let events: Vec<ExclusionEvent> = records
        .iter()
        .enumerate()
        .filter_map(|(i, rec)| {
            let entity_id = &entity_id_by_rep[&cluster_of[i]];
            build_event(rec, entity_id)
        })
        .collect();

    let candidates = candidate_lines(resolved, cluster_of, entity_id_by_rep, rejected);
    let sources: BTreeSet<String> = records.iter().map(|r| r.source_id.clone()).collect();

    let mut out = String::new();
    push_line(
        &mut out,
        "meta",
        &meta_value(entities.len(), events.len(), &sources, &coverage),
    );
    for entity in &entities {
        push_line(&mut out, "entity", entity);
    }
    for event in &events {
        push_line(&mut out, "exclusion_event", event);
    }
    for candidate in &candidates {
        push_line(&mut out, "identity_link_candidate", candidate);
    }
    out
}

fn meta_value(
    entity_count: usize,
    event_count: usize,
    sources: &BTreeSet<String>,
    coverage: &BTreeSet<String>,
) -> Value {
    serde_json::json!({
        "export_version": "1.0.0",
        "exported_at": Utc::now().to_rfc3339(),
        "pipeline_version": "0.2.0-rust",
        "entity_count": entity_count,
        "exclusion_event_count": event_count,
        "sources_ingested": sources.iter().collect::<Vec<_>>(),
        "jurisdiction_coverage": coverage.iter().collect::<Vec<_>>(),
    })
}

fn build_entity(
    entity_id: String,
    members: &[usize],
    records: &[NormalizedExclusion],
    npis: &[Option<String>],
    nppes: &BTreeMap<String, NppesInfo>,
) -> Entity {
    let canonical_name = most_common(
        members
            .iter()
            .map(|&i| records[i].person_or_entity_name.clone()),
    )
    .unwrap_or_default();
    let canonical_state = most_common(members.iter().filter_map(|&i| records[i].state.clone()));
    let member_npis: BTreeSet<String> = members.iter().filter_map(|&i| npis[i].clone()).collect();

    let mut npi_active: Option<bool> = None;
    let mut npi_deactivation_date: Option<String> = None;
    for npi in &member_npis {
        if let Some(info) = nppes.get(npi) {
            npi_active = Some(npi_active.unwrap_or(false) || info.active);
            // Normalize NPPES MM/DD/YYYY -> RFC3339; keep the earliest.
            if let Some(rfc) = to_rfc3339(info.deactivation_date.as_deref()) {
                if npi_deactivation_date
                    .as_ref()
                    .map_or(true, |cur| rfc < *cur)
                {
                    npi_deactivation_date = Some(rfc);
                }
            }
        }
    }

    let has_npi = !member_npis.is_empty();
    Entity {
        entity_id,
        entity_type: entity_type(&canonical_name),
        canonical_name,
        canonical_state,
        npis: member_npis.into_iter().collect(),
        npi_active,
        npi_deactivation_date,
        resolution_confidence: if has_npi || members.len() == 1 {
            1.0
        } else {
            0.85
        },
    }
}

fn build_event(rec: &NormalizedExclusion, entity_id: &str) -> Option<ExclusionEvent> {
    let exclusion_date = to_rfc3339(rec.exclusion_date.as_deref());
    let reinstatement_date = to_rfc3339(rec.reinstatement_date.as_deref());
    if exclusion_date.is_none() && reinstatement_date.is_none() {
        return None;
    }
    let status = if rec.indefinite_exclusion {
        ExclusionStatus::Indefinite
    } else if reinstatement_date.is_some() {
        ExclusionStatus::Reinstated
    } else {
        ExclusionStatus::Active
    };
    let authority = authority_for(&rec.source_id);
    let record_id = rec.record_id();
    let event_id = event_id(
        entity_id,
        &rec.source_id,
        exclusion_date.as_deref(),
        &record_id,
    );
    Some(ExclusionEvent {
        event_id,
        entity_id: entity_id.to_string(),
        authority,
        exclusion_type: rec.provider_type.clone(),
        exclusion_date,
        reinstatement_date,
        status,
        state: rec.state.clone(),
        source_id: rec.source_id.clone(),
        source_record_id: record_id,
        source_snapshot_hash: rec.source_snapshot_hash.clone(),
        observed_at: rec
            .observed_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
    })
}

fn candidate_lines(
    resolved: &resolve::ResolveResult,
    cluster_of: &[usize],
    entity_id_by_rep: &BTreeMap<usize, String>,
    rejected: &BTreeSet<(String, String)>,
) -> Vec<LinkCandidateLine> {
    let mut best: BTreeMap<(String, String), LinkCandidateLine> = BTreeMap::new();
    for cand in &resolved.candidates {
        let a = entity_id_by_rep[&cluster_of[cand.a]].clone();
        let b = entity_id_by_rep[&cluster_of[cand.b]].clone();
        if a == b {
            continue;
        }
        let key = if a < b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        // A reviewer rejected this link — keep the entities separate and stop
        // surfacing it in the review queue.
        if rejected.contains(&key) {
            continue;
        }
        let replace = best
            .get(&key)
            .map_or(true, |e| cand.confidence > e.confidence);
        if replace {
            best.insert(
                key.clone(),
                LinkCandidateLine {
                    entity_a: key.0,
                    entity_b: key.1,
                    confidence: cand.confidence,
                    signals: cand.signals.clone(),
                },
            );
        }
    }
    best.into_values().collect()
}

/// Group `cluster_of[record] = rep` into `rep -> member record indices`.
fn group_clusters(cluster_of: &[usize]) -> BTreeMap<usize, Vec<usize>> {
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &rep) in cluster_of.iter().enumerate() {
        clusters.entry(rep).or_default().push(i);
    }
    clusters
}

fn entity_ids(
    clusters: &BTreeMap<usize, Vec<usize>>,
    records: &[NormalizedExclusion],
    npis: &[Option<String>],
) -> BTreeMap<usize, String> {
    clusters
        .iter()
        .map(|(&rep, members)| (rep, entity_id_for(members, records, npis)))
        .collect()
}

/// Force-merge clusters for confirmed identity links (a human verdict overrides
/// the resolver's threshold). Mutates `cluster_of` in place; returns how many
/// confirmed links were applied.
fn apply_confirmed(
    cluster_of: &mut [usize],
    entity_id_by_rep: &BTreeMap<usize, String>,
    confirmed: &BTreeSet<(String, String)>,
) -> usize {
    if confirmed.is_empty() {
        return 0;
    }
    let rep_by_id: HashMap<&str, usize> = entity_id_by_rep
        .iter()
        .map(|(&rep, id)| (id.as_str(), rep))
        .collect();

    let n = cluster_of.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut applied = 0;
    for (a, b) in confirmed {
        if let (Some(&ra), Some(&rb)) = (rep_by_id.get(a.as_str()), rep_by_id.get(b.as_str())) {
            if uf_union(&mut parent, ra, rb) {
                applied += 1;
            }
        }
    }
    for rep in cluster_of.iter_mut() {
        *rep = uf_find(&mut parent, *rep);
    }
    applied
}

fn uf_find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn uf_union(parent: &mut [usize], a: usize, b: usize) -> bool {
    let (ra, rb) = (uf_find(parent, a), uf_find(parent, b));
    if ra == rb {
        return false;
    }
    // Keep the smaller index as representative for determinism.
    parent[ra.max(rb)] = ra.min(rb);
    true
}

fn entity_id_for(
    members: &[usize],
    _records: &[NormalizedExclusion],
    npis: &[Option<String>],
) -> String {
    let member_npis: BTreeSet<&str> = members.iter().filter_map(|&i| npis[i].as_deref()).collect();
    if let Some(npi) = member_npis.iter().next() {
        return (*npi).to_string();
    }
    let name = most_common(
        members
            .iter()
            .map(|&i| resolve::normalize_name(&_records[i].person_or_entity_name)),
    )
    .unwrap_or_default();
    format!("name:{name}")
}

fn most_common<I: IntoIterator<Item = String>>(values: I) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for v in values {
        if !v.is_empty() {
            *counts.entry(v).or_default() += 1;
        }
    }
    // Most frequent, then longest, then lexicographically smallest.
    counts
        .into_iter()
        .max_by(|(ka, ca), (kb, cb)| ca.cmp(cb).then(ka.len().cmp(&kb.len())).then(kb.cmp(ka)))
        .map(|(k, _)| k)
}

fn entity_type(name: &str) -> EntityType {
    let toks: BTreeSet<String> = resolve::normalize_name(name)
        .split_whitespace()
        .map(String::from)
        .collect();
    if toks.iter().any(|t| ORG_TOKENS.contains(&t.as_str())) {
        EntityType::Organization
    } else {
        EntityType::Individual
    }
}

fn authority_for(source_id: &str) -> ExclusionAuthority {
    if source_id.starts_with("hawaii_medquest") {
        ExclusionAuthority::StateMedicaid
    } else if source_id.starts_with("sam_gov") {
        ExclusionAuthority::SamGov
    } else {
        ExclusionAuthority::HhsOig
    }
}

fn event_id(entity_id: &str, source_id: &str, excl: Option<&str>, record_id: &str) -> String {
    let authority = authority_for(source_id);
    let key = format!(
        "{entity_id}|{authority:?}|{}|{record_id}",
        excl.unwrap_or("")
    );
    hex::encode(&blake3::hash(key.as_bytes()).as_bytes()[..16])
}

/// Normalize ISO-8601 or US `MM/DD/YY` dates to RFC 3339 (so the detection
/// engine, which parses RFC 3339, can read every source uniformly).
fn to_rfc3339(raw: Option<&str>) -> Option<String> {
    let text = raw?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.with_timezone(&Utc).to_rfc3339());
    }
    for fmt in ["%m/%d/%y", "%m/%d/%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, fmt) {
            let naive = date.and_hms_opt(0, 0, 0)?;
            return Some(Utc.from_utc_datetime(&naive).to_rfc3339());
        }
    }
    None
}

/// Append `value` as a JSON object line with a leading `"kind"` tag, splicing
/// the tag onto the serialized object directly (no intermediate `Value` tree —
/// this runs once per entity/event/candidate, so it is on the hot path).
fn push_line<T: Serialize>(out: &mut String, kind: &str, value: &T) {
    let Ok(json) = serde_json::to_string(value) else {
        return;
    };
    out.push_str("{\"kind\":\"");
    out.push_str(kind);
    out.push('"');
    if json.len() > 2 {
        // json is `{...}`; keep everything after the opening brace.
        out.push(',');
        out.push_str(&json[1..]);
    } else {
        out.push('}');
    }
    out.push('\n');
}
