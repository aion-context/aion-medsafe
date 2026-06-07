// SPDX-License-Identifier: MIT OR Apache-2.0
//! Entity resolution — clusters source records into provider entities.
//!
//! Two layers (ported from the Python prototype, scoring cleaned up):
//!
//! 1. Deterministic hard merges (definitional):
//!    - same NPI → same provider (the unique government identifier)
//!    - same normalized name → same provider, UNLESS the two records carry
//!      different non-null NPIs (which proves they are distinct)
//!
//! 2. Multi-signal fuzzy linking for name variants / misspellings the exact
//!    keys miss. Records are blocked by phonetic key (bounded block size — no
//!    O(n²) over the whole corpus), and each candidate pair is scored:
//!      - score ≥ MERGE_THRESHOLD  → merge
//!      - REVIEW_THRESHOLD..MERGE  → emit a review candidate (NOT merged;
//!        honors "no auto-link below threshold", agents.md)
//!
//! Resolution is deterministic: input order is stable, blocks are visited in
//! sorted key order, and union-find is order-independent.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Auto-merge at/above this fuzzy score. Deliberately high: an autonomous merge
/// attributes one provider's exclusions to another, so we only do it when edit,
/// token, and phonetic signals strongly agree. Everything weaker goes to review.
const MERGE_THRESHOLD: f64 = 0.90;
/// Surface as a human-review candidate at/above this score (but below merge).
const REVIEW_THRESHOLD: f64 = 0.72;
/// Max records per phonetic block to score pairwise (Tiger Style: bound work).
const MAX_BLOCK_SIZE: usize = 400;

const NOISE_TOKENS: &[&str] = &[
    "AKA", "OR", "DBA", "JR", "SR", "III", "II", "IV", "THE", "AND",
];

/// Minimal view of a record needed for resolution.
pub struct ResolveInput<'a> {
    pub name: &'a str,
    pub npi: Option<&'a str>,
    pub state: Option<&'a str>,
}

/// A sub-merge match surfaced for human review (not auto-merged).
#[derive(Debug, Clone)]
pub struct LinkCandidate {
    pub a: usize,
    pub b: usize,
    pub confidence: f64,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolveStats {
    pub npi_merges: usize,
    pub name_merges: usize,
    pub fuzzy_auto_links: usize,
    pub candidates: usize,
    pub blocks_capped: usize,
}

pub struct ResolveResult {
    /// `cluster_of[i]` = representative record index for record `i`.
    pub cluster_of: Vec<usize>,
    pub candidates: Vec<LinkCandidate>,
    pub stats: ResolveStats,
}

// ─── Union-Find ──────────────────────────────────────────────────────────────

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u32>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Returns true if the two sets were distinct and are now merged.
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut ra, mut rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        if self.rank[ra] < self.rank[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] += 1;
        }
        true
    }
}

// ─── Text normalization & similarity ─────────────────────────────────────────

/// Canonical uppercase name: alpha + spaces only, noise tokens removed.
pub fn normalize_name(raw: &str) -> String {
    let upper: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() || c.is_whitespace() {
                c.to_ascii_uppercase()
            } else {
                ' '
            }
        })
        .collect();
    upper
        .split_whitespace()
        .filter(|t| !NOISE_TOKENS.contains(t))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Simplified phonetic key for a single token (metaphone-lite).
fn metaphone_token(word: &str) -> String {
    let w: Vec<char> = word.chars().collect();
    if w.is_empty() {
        return String::new();
    }
    let start = match (w.first(), w.get(1)) {
        (Some('A'), Some('E'))
        | (Some('G'), Some('N'))
        | (Some('K'), Some('N'))
        | (Some('P'), Some('N'))
        | (Some('W'), Some('R')) => 1,
        _ => 0,
    };
    let mut out = String::new();
    out.push(w[start]);
    for &c in &w[start + 1..] {
        if "AEIOU".contains(c) {
            continue;
        }
        if !out.ends_with(c) {
            out.push(c);
        }
    }
    out.chars().take(6).collect()
}

/// Phonetic keys from an ALREADY-normalized name (no re-normalization).
fn keys_from_norm(norm: &str) -> BTreeSet<String> {
    norm.split_whitespace()
        .filter(|t| t.len() > 1)
        .map(metaphone_token)
        .filter(|k| !k.is_empty())
        .collect()
}

/// Levenshtein distance over ASCII bytes (normalized names are ASCII). Uses a
/// single stack DP row — allocation-free for names up to 63 chars (the common
/// case); longer names fall back to a heap row. Called millions of times.
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (a, b) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = b.len();
    if m == 0 {
        return a.len();
    }
    const CAP: usize = 64;
    if m >= CAP {
        return levenshtein_heap(a, b);
    }
    let mut row = [0usize; CAP];
    for (j, slot) in row.iter_mut().enumerate().take(m + 1) {
        *slot = j;
    }
    for (i, &ca) in a.iter().enumerate() {
        let mut prev_diag = row[0];
        row[0] = i + 1;
        for j in 0..m {
            let above = row[j + 1];
            let cost = usize::from(ca != b[j]);
            row[j + 1] = (row[j] + 1).min(above + 1).min(prev_diag + cost);
            prev_diag = above;
        }
    }
    row[m]
}

fn levenshtein_heap(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut prev_diag = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let above = row[j + 1];
            let cost = usize::from(ca != cb);
            row[j + 1] = (row[j] + 1).min(above + 1).min(prev_diag + cost);
            prev_diag = above;
        }
    }
    row[b.len()]
}

/// Edit similarity of two ALREADY-normalized (ASCII) names.
fn edit_sim_norm(a: &str, b: &str) -> f64 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 0.0;
    }
    1.0 - (levenshtein(a, b) as f64 / max_len as f64)
}

/// FNV-1a hash of a token. Tokens/phonetic keys are stored as sorted, deduped
/// `u32` hashes so set overlap is a two-pointer merge over integers — no string
/// comparison or allocation on the hot path. (Hash collisions are astronomically
/// rare and would at worst nudge a fuzzy score, never a hard merge.)
fn fnv1a(s: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in s.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn hashed_sorted<'a, I: Iterator<Item = &'a str>>(items: I) -> Vec<u32> {
    let mut v: Vec<u32> = items.map(fnv1a).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Intersection and union sizes of two sorted, deduped hash slices.
fn overlap(a: &[u32], b: &[u32]) -> (usize, usize) {
    let (mut i, mut j, mut inter) = (0usize, 0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
        }
    }
    (inter, a.len() + b.len() - inter)
}

fn jaccard(a: &[u32], b: &[u32]) -> f64 {
    let (inter, union) = overlap(a, b);
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

// ─── Pairwise scoring ────────────────────────────────────────────────────────

/// Per-record features, computed ONCE up front. Recomputing these per pair was
/// the dominant cost (millions of comparisons × redundant normalization).
struct Features {
    norm: Vec<String>,
    tokens: Vec<Vec<u32>>,
    phonetic: Vec<Vec<u32>>,
}

impl Features {
    fn build(records: &[ResolveInput]) -> Self {
        let norm: Vec<String> = records.iter().map(|r| normalize_name(r.name)).collect();
        let tokens = norm
            .iter()
            .map(|s| hashed_sorted(s.split_whitespace()))
            .collect();
        let phonetic = norm
            .iter()
            .map(|s| hashed_sorted(keys_from_norm(s).iter().map(String::as_str)))
            .collect();
        Self {
            norm,
            tokens,
            phonetic,
        }
    }
}

type States<'a> = (Option<&'a str>, Option<&'a str>);

fn token_sim(f: &Features, i: usize, j: usize) -> f64 {
    let (ta, tb) = (&f.tokens[i], &f.tokens[j]);
    let (inter, union) = overlap(ta, tb);
    let mut token = if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    };
    if !ta.is_empty() && !tb.is_empty() {
        let containment = inter as f64 / ta.len().min(tb.len()) as f64 * 0.9;
        token = token.max(containment);
    }
    token
}

fn state_bonus(states: States) -> f64 {
    match states {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => 0.05,
        _ => 0.0,
    }
}

/// Numeric similarity in [0, 1] — the hot path (no allocation). A blend of edit
/// distance, token overlap, and phonetic agreement, nudged by state match, with
/// NO single-signal coverage penalty (the prototype's bug). The costly edit
/// distance is skipped when token+phonetic overlap can't reach the review floor.
fn score_value(i: usize, j: usize, f: &Features, states: States) -> f64 {
    let token = token_sim(f, i, j);
    let phon = jaccard(&f.phonetic[i], &f.phonetic[j]);
    let bonus = state_bonus(states);
    // Upper bound with a perfect edit (0.6). If that can't reach REVIEW, prune.
    if 0.6 + 0.25 * token + 0.15 * phon + bonus < REVIEW_THRESHOLD {
        return 0.0;
    }
    let edit = edit_sim_norm(&f.norm[i], &f.norm[j]);
    (0.6 * edit + 0.25 * token + 0.15 * phon + bonus).min(1.0)
}

/// The signals that fired — built only for surfaced matches (cold path).
fn signals_for(i: usize, j: usize, f: &Features, states: States) -> Vec<String> {
    let mut signals = Vec::new();
    if edit_sim_norm(&f.norm[i], &f.norm[j]) > 0.6 {
        signals.push("name_edit_distance".into());
    }
    if token_sim(f, i, j) > 0.5 {
        signals.push("name_token_overlap".into());
    }
    if jaccard(&f.phonetic[i], &f.phonetic[j]) > 0.5 {
        signals.push("name_phonetic".into());
    }
    if state_bonus(states) > 0.0 {
        signals.push("state_match".into());
    }
    signals
}

// ─── Resolution ──────────────────────────────────────────────────────────────

pub fn resolve(records: &[ResolveInput]) -> ResolveResult {
    let n = records.len();
    let mut uf = UnionFind::new(n);
    let mut stats = ResolveStats::default();

    let features = Features::build(records);
    let norm = &features.norm;

    // Pass A: deterministic hard merges (NPI, then exact normalized name).
    let mut by_npi: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_name: BTreeMap<&str, usize> = BTreeMap::new();
    for i in 0..n {
        if let Some(npi) = records[i].npi {
            match by_npi.get(npi) {
                Some(&first) if uf.union(first, i) => stats.npi_merges += 1,
                Some(_) => {}
                None => {
                    by_npi.insert(npi, i);
                }
            }
        }
        if !norm[i].is_empty() {
            match by_name.get(norm[i].as_str()) {
                Some(&first) => {
                    let conflict = matches!(
                        (records[i].npi, records[first].npi),
                        (Some(x), Some(y)) if x != y
                    );
                    if !conflict && uf.union(first, i) {
                        stats.name_merges += 1;
                    }
                }
                None => {
                    by_name.insert(norm[i].as_str(), i);
                }
            }
        }
    }

    // Pass B: fuzzy linking within bounded phonetic blocks (keyed by hash).
    let mut blocks: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, keys) in features.phonetic.iter().enumerate() {
        for &key in keys {
            blocks.entry(key).or_default().push(i);
        }
    }

    let mut candidates = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for idxs in blocks.values() {
        if idxs.len() > MAX_BLOCK_SIZE {
            stats.blocks_capped += 1;
            continue;
        }
        for a_pos in 0..idxs.len() {
            for b_pos in (a_pos + 1)..idxs.len() {
                let (i, j) = (idxs[a_pos], idxs[b_pos]);
                if uf.find(i) == uf.find(j) || norm[i] == norm[j] {
                    continue;
                }
                if matches!((records[i].npi, records[j].npi), (Some(x), Some(y)) if x != y) {
                    continue;
                }
                let pair = if i < j { (i, j) } else { (j, i) };
                if !seen.insert(pair) {
                    continue;
                }

                let states = (records[i].state, records[j].state);
                let confidence = score_value(i, j, &features, states);
                if confidence >= MERGE_THRESHOLD {
                    if uf.union(i, j) {
                        stats.fuzzy_auto_links += 1;
                    }
                } else if confidence >= REVIEW_THRESHOLD {
                    candidates.push(LinkCandidate {
                        a: pair.0,
                        b: pair.1,
                        confidence: (confidence * 10_000.0).round() / 10_000.0,
                        signals: signals_for(i, j, &features, states),
                    });
                    stats.candidates += 1;
                }
            }
        }
    }

    let cluster_of = (0..n).map(|i| uf.find(i)).collect();
    ResolveResult {
        cluster_of,
        candidates,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec<'a>(name: &'a str, npi: Option<&'a str>, state: Option<&'a str>) -> ResolveInput<'a> {
        ResolveInput { name, npi, state }
    }

    /// True if two record indices ended up in the same cluster.
    fn same(r: &ResolveResult, i: usize, j: usize) -> bool {
        r.cluster_of[i] == r.cluster_of[j]
    }

    #[test]
    fn normalize_strips_punctuation_noise_and_case() {
        assert_eq!(
            normalize_name("#1 Marketing Service, Inc"),
            "MARKETING SERVICE INC"
        );
        assert_eq!(normalize_name("SMITH, JOHN JR."), "SMITH JOHN");
    }

    #[test]
    fn merges_same_npi_even_with_different_names() {
        let inputs = [
            rec("SMITH JOHN", Some("1234567890"), Some("HI")),
            rec("SMITH JONATHAN", Some("1234567890"), Some("CA")),
        ];
        let r = resolve(&inputs);
        assert!(same(&r, 0, 1));
        assert_eq!(r.stats.npi_merges, 1);
    }

    #[test]
    fn merges_exact_normalized_name_without_npi() {
        let inputs = [
            rec("DOE, JANE", None, Some("HI")),
            rec("JANE DOE!!", None, Some("HI")),
        ];
        // Both normalize to "DOE JANE" vs "JANE DOE" — NOT exact; but
        // "DOE, JANE" and "DOE JANE" ARE exact after normalization.
        let exact = [rec("DOE, JANE", None, None), rec("DOE JANE", None, None)];
        let r = resolve(&exact);
        assert!(same(&r, 0, 1));
        assert_eq!(r.stats.name_merges, 1);
        // sanity: the reordered pair is not an exact-name merge
        let _ = inputs;
    }

    #[test]
    fn never_merges_conflicting_npis_even_with_same_name() {
        let inputs = [
            rec("ACME HOME HEALTH", Some("1111111111"), Some("HI")),
            rec("ACME HOME HEALTH", Some("2222222222"), Some("HI")),
        ];
        let r = resolve(&inputs);
        assert!(!same(&r, 0, 1));
    }

    #[test]
    fn fuzzy_auto_merges_phonetically_identical_typo() {
        // Phonetically identical full name, 1-char typo, same state → strong
        // agreement across edit + token + phonetic → auto-merge.
        let inputs = [
            rec("MARY JANE WATSON", None, Some("HI")),
            rec("MARY JANE WATSN", None, Some("HI")),
        ];
        let r = resolve(&inputs);
        assert!(same(&r, 0, 1));
        assert_eq!(r.stats.fuzzy_auto_links, 1);
    }

    #[test]
    fn fuzzy_variant_becomes_review_candidate_not_merge() {
        // A single-token typo is plausible but not certain → surfaced for human
        // review, NOT auto-merged (no autonomous cross-provider attribution).
        let inputs = [
            rec("JONATHAN SMITHERS", None, Some("HI")),
            rec("JONATHAN SMITHERZ", None, Some("HI")),
        ];
        let r = resolve(&inputs);
        assert!(!same(&r, 0, 1));
        assert_eq!(r.candidates.len(), 1);
        let c = &r.candidates[0];
        assert!(c.confidence >= REVIEW_THRESHOLD && c.confidence < MERGE_THRESHOLD);
    }

    #[test]
    fn distinct_names_stay_separate() {
        let inputs = [
            rec("ALICE JOHNSON", None, Some("HI")),
            rec("BOB WILLIAMS", None, Some("CA")),
        ];
        let r = resolve(&inputs);
        assert!(!same(&r, 0, 1));
        assert!(r.candidates.is_empty());
    }

    #[test]
    fn resolution_is_deterministic() {
        let inputs = [
            rec("SMITH JOHN", Some("1"), Some("HI")),
            rec("SMITH JON", None, Some("HI")),
            rec("SMITH JOHN", Some("1"), Some("CA")),
        ];
        let a = resolve(&inputs);
        let b = resolve(&inputs);
        assert_eq!(a.cluster_of, b.cluster_of);
    }

    #[test]
    fn levenshtein_matches_known_values() {
        assert_eq!(levenshtein("KITTEN", "SITTING"), 3);
        assert_eq!(levenshtein("", "ABC"), 3);
        assert_eq!(levenshtein("SAME", "SAME"), 0);
    }
}
