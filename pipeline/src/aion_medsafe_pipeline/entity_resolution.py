"""Entity Resolution Engine for AION-MEDSAFE.

De-risks the low NPI coverage and name mismatch problems by using
multi-signal probabilistic matching. No single signal is authoritative alone.

Technique stack:
- Deterministic: exact NPI match (highest confidence)
- Phonetic: Double Metaphone for name similarity across misspellings
- Edit distance: Levenshtein for near-misses and truncations
- Token overlap: Jaccard similarity on name tokens
- Contextual: state + provider type + date proximity as boosters

Every match produces a confidence score. Only human-reviewable matches
above threshold are promoted to links in the Provider Trust Graph.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any


class MatchSignal(StrEnum):
    NPI_EXACT = "npi_exact"
    NAME_EXACT = "name_exact"
    NAME_PHONETIC = "name_phonetic"
    NAME_EDIT_DISTANCE = "name_edit_distance"
    NAME_TOKEN_OVERLAP = "name_token_overlap"
    STATE_MATCH = "state_match"
    PROVIDER_TYPE_MATCH = "provider_type_match"
    DATE_PROXIMITY = "date_proximity"


# Signal weights — tunable, should be calibrated against known-true pairs
SIGNAL_WEIGHTS: dict[MatchSignal, float] = {
    MatchSignal.NPI_EXACT: 1.0,
    MatchSignal.NAME_EXACT: 0.9,
    MatchSignal.NAME_PHONETIC: 0.5,
    MatchSignal.NAME_EDIT_DISTANCE: 0.4,
    MatchSignal.NAME_TOKEN_OVERLAP: 0.35,
    MatchSignal.STATE_MATCH: 0.15,
    MatchSignal.PROVIDER_TYPE_MATCH: 0.1,
    MatchSignal.DATE_PROXIMITY: 0.05,
}

# Thresholds
CONFIDENCE_AUTO_LINK = 0.95  # auto-link without human review
CONFIDENCE_REVIEW = 0.55     # flag for human review
CONFIDENCE_REJECT = 0.55     # below this, no link


@dataclass(frozen=True)
class MatchEvidence:
    signal: MatchSignal
    score: float  # 0.0 to 1.0
    detail: str


@dataclass
class MatchCandidate:
    source_a_id: str
    source_b_id: str
    evidences: list[MatchEvidence] = field(default_factory=list)

    # Identity signals (must have at least one for a meaningful match)
    _IDENTITY_SIGNALS = {
        MatchSignal.NPI_EXACT,
        MatchSignal.NAME_EXACT,
        MatchSignal.NAME_PHONETIC,
        MatchSignal.NAME_EDIT_DISTANCE,
        MatchSignal.NAME_TOKEN_OVERLAP,
    }

    @property
    def has_identity_signal(self) -> bool:
        """True if at least one name or NPI signal fired."""
        return any(e.signal in self._IDENTITY_SIGNALS for e in self.evidences)

    @property
    def confidence(self) -> float:
        if not self.evidences:
            return 0.0
        # Contextual-only matches (state, provider type) get capped at low confidence
        if not self.has_identity_signal:
            return 0.1
        # Weighted average of evidence scores, normalized by weights of present signals
        weighted_sum = sum(
            e.score * SIGNAL_WEIGHTS[e.signal] for e in self.evidences
        )
        max_possible = sum(
            SIGNAL_WEIGHTS[e.signal] for e in self.evidences
        )
        raw = weighted_sum / max_possible if max_possible > 0 else 0.0
        # Scale by coverage: how many signal categories fired vs possible identity signals
        identity_fired = sum(1 for e in self.evidences if e.signal in self._IDENTITY_SIGNALS)
        coverage_bonus = min(identity_fired / 2.0, 1.0)  # 2+ identity signals = full coverage
        return raw * coverage_bonus

    @property
    def disposition(self) -> str:
        c = self.confidence
        if not self.has_identity_signal:
            return "reject"
        if c >= CONFIDENCE_AUTO_LINK:
            return "auto_link"
        elif c >= CONFIDENCE_REVIEW:
            return "human_review"
        return "reject"


# ─── Name Normalization ───────────────────────────────────────────────────────


_NOISE_TOKENS = {"AKA", "A.K.A.", "OR", "DBA", "D/B/A", "JR", "JR.", "SR", "SR.", "III", "II", "IV"}
_STRIP_CHARS = re.compile(r"[^A-Z\s]")


def normalize_name(raw: str) -> str:
    """Canonical uppercase name with noise tokens removed."""
    upper = raw.upper().strip()
    upper = _STRIP_CHARS.sub("", upper)
    tokens = [t for t in upper.split() if t and t not in _NOISE_TOKENS]
    return " ".join(tokens)


def name_tokens(name: str) -> set[str]:
    return set(normalize_name(name).split())


# ─── Phonetic Matching (Double Metaphone, simplified) ─────────────────────────


def _metaphone_simple(word: str) -> str:
    """Simplified phonetic hash. Production should use full Double Metaphone."""
    word = word.upper().strip()
    if not word:
        return ""
    # Drop silent leading letters
    if word[:2] in ("AE", "GN", "KN", "PN", "WR"):
        word = word[1:]
    # Simplistic vowel-stripping after first char
    first = word[0]
    consonants = re.sub(r"[AEIOU]", "", word[1:])
    # Collapse repeated consonants
    result = first
    for c in consonants:
        if not result or c != result[-1]:
            result += c
    return result[:6]


def phonetic_keys(name: str) -> set[str]:
    """Generate phonetic keys for each token in a name."""
    tokens = normalize_name(name).split()
    return {_metaphone_simple(t) for t in tokens if len(t) > 1}


# ─── Edit Distance ────────────────────────────────────────────────────────────


def levenshtein(s: str, t: str) -> int:
    """Standard Levenshtein distance."""
    if len(s) < len(t):
        return levenshtein(t, s)
    if len(t) == 0:
        return len(s)

    prev = list(range(len(t) + 1))
    for i, sc in enumerate(s):
        curr = [i + 1]
        for j, tc in enumerate(t):
            cost = 0 if sc == tc else 1
            curr.append(min(curr[j] + 1, prev[j + 1] + 1, prev[j] + cost))
        prev = curr
    return prev[-1]


def name_edit_similarity(a: str, b: str) -> float:
    """Normalized edit distance similarity (0.0 to 1.0)."""
    na = normalize_name(a)
    nb = normalize_name(b)
    if not na or not nb:
        return 0.0
    dist = levenshtein(na, nb)
    max_len = max(len(na), len(nb))
    return 1.0 - (dist / max_len)


# ─── Token Overlap (Jaccard) ──────────────────────────────────────────────────


def jaccard_similarity(a: set, b: set) -> float:
    if not a and not b:
        return 0.0
    intersection = a & b
    union = a | b
    return len(intersection) / len(union)


# ─── Composite Matcher ────────────────────────────────────────────────────────


def match_records(record_a: dict[str, Any], record_b: dict[str, Any]) -> MatchCandidate:
    """Compare two records from different sources and produce a scored match candidate."""
    candidate = MatchCandidate(
        source_a_id=record_a.get("source_record_id", "unknown"),
        source_b_id=record_b.get("source_record_id", "unknown"),
    )

    # NPI exact match
    npi_a = record_a.get("npi")
    npi_b = record_b.get("npi")
    if npi_a and npi_b and npi_a == npi_b:
        candidate.evidences.append(MatchEvidence(
            signal=MatchSignal.NPI_EXACT,
            score=1.0,
            detail=f"NPI={npi_a}",
        ))

    # Name matching
    name_a = record_a.get("person_or_entity_name", "")
    name_b = record_b.get("person_or_entity_name", "")

    if name_a and name_b:
        norm_a = normalize_name(name_a)
        norm_b = normalize_name(name_b)

        # Exact
        if norm_a == norm_b:
            candidate.evidences.append(MatchEvidence(
                signal=MatchSignal.NAME_EXACT,
                score=1.0,
                detail=f"{norm_a}=={norm_b}",
            ))
        else:
            # Edit distance
            edit_sim = name_edit_similarity(name_a, name_b)
            if edit_sim > 0.65:
                candidate.evidences.append(MatchEvidence(
                    signal=MatchSignal.NAME_EDIT_DISTANCE,
                    score=edit_sim,
                    detail=f"edit_sim={edit_sim:.3f}",
                ))

            # Phonetic
            phon_a = phonetic_keys(name_a)
            phon_b = phonetic_keys(name_b)
            if phon_a and phon_b:
                phon_overlap = jaccard_similarity(phon_a, phon_b)
                if phon_overlap > 0.5:
                    candidate.evidences.append(MatchEvidence(
                        signal=MatchSignal.NAME_PHONETIC,
                        score=phon_overlap,
                        detail=f"phonetic_jaccard={phon_overlap:.3f}",
                    ))

            # Token overlap
            tokens_a = name_tokens(name_a)
            tokens_b = name_tokens(name_b)
            token_sim = jaccard_similarity(tokens_a, tokens_b)
            # Also check containment (one name's tokens are subset of the other)
            if tokens_a and tokens_b:
                containment = len(tokens_a & tokens_b) / min(len(tokens_a), len(tokens_b))
                token_sim = max(token_sim, containment * 0.9)
            if token_sim > 0.3:
                candidate.evidences.append(MatchEvidence(
                    signal=MatchSignal.NAME_TOKEN_OVERLAP,
                    score=token_sim,
                    detail=f"token_jaccard={token_sim:.3f}",
                ))

    # State match
    state_a = record_a.get("state")
    state_b = record_b.get("state")
    if state_a and state_b and state_a == state_b:
        candidate.evidences.append(MatchEvidence(
            signal=MatchSignal.STATE_MATCH,
            score=1.0,
            detail=f"state={state_a}",
        ))

    return candidate
