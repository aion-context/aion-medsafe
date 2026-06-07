"""Trust Graph export — the sealed handoff to the Rust system.

Reads the per-source normalized NDJSON, resolves entities by NPI (falling back
to normalized name), reconstructs exclusion/reinstatement events across federal
and state sources, enriches with NPPES NPI status, and emits a typed-NDJSON
``TrustGraphExport``:

    {"kind": "meta", ...}
    {"kind": "entity", ...}
    {"kind": "exclusion_event", ...}

That file is then sealed into a ``.aion`` by the Rust system
(``aion-medsafe seal-graph``) so the signal engine only ever consumes a
verified payload. See ``schema.py`` for the field contract and
``.claude/rules/architecture-decisions.md`` (ADR-005) for the NDJSON rationale.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
from datetime import UTC, datetime
from typing import Any, TextIO

from aion_medsafe_pipeline.entity_resolution import normalize_name

PIPELINE_VERSION = "0.1.0"

# source_id prefix -> ExclusionAuthority value (schema.py)
_AUTHORITY_BY_SOURCE: dict[str, str] = {
    "hhs_oig_leie": "hhs_oig",
    "hawaii_medquest": "state_medicaid",
    "sam_gov": "sam_gov",
}

# Tokens that mark a name as an organization rather than an individual.
_ORG_TOKENS = {
    "INC", "LLC", "LLP", "CORP", "CO", "COMPANY", "CENTER", "CENTERS",
    "SERVICE", "SERVICES", "HOSPITAL", "CLINIC", "PHARMACY", "LAB", "LABS",
    "LABORATORY", "GROUP", "ASSOC", "ASSOCIATES", "HEALTH", "HOME", "AGENCY",
    "FOUNDATION", "SYSTEMS", "SOLUTIONS", "ENTERPRISES", "MARKETING",
}


def _authority_for(source_id: str) -> str:
    for prefix, authority in _AUTHORITY_BY_SOURCE.items():
        if source_id.startswith(prefix):
            return authority
    return "hhs_oig"


def _entity_type(name: str) -> str:
    tokens = set(normalize_name(name).split())
    return "organization" if tokens & _ORG_TOKENS else "individual"


def parse_any_date(raw: str | None) -> datetime | None:
    """Parse ISO-8601 or US ``MM/DD/YY`` dates into a UTC datetime."""
    if not raw:
        return None
    text = raw.strip()
    if not text:
        return None
    # ISO first (LEIE / supplements)
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
        return parsed if parsed.tzinfo else parsed.replace(tzinfo=UTC)
    except ValueError:
        pass
    # US short date (Hawaii Med-QUEST)
    for fmt in ("%m/%d/%y", "%m/%d/%Y"):
        try:
            return datetime.strptime(text, fmt).replace(tzinfo=UTC)
        except ValueError:
            continue
    return None


def _iso(dt: datetime | None) -> str | None:
    return dt.isoformat() if dt else None


def _event_id(entity_id: str, authority: str, excl: str | None, record_id: str) -> str:
    digest = hashlib.blake2b(
        f"{entity_id}|{authority}|{excl}|{record_id}".encode(), digest_size=16
    )
    return digest.hexdigest()


def _load_nppes_active(nppes_path: pathlib.Path) -> dict[str, bool]:
    """Map NPI -> is-active (NPPES status 'A')."""
    active: dict[str, bool] = {}
    if not nppes_path.exists():
        return active
    with open(nppes_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            npi = str(rec.get("number") or "").strip()
            if npi:
                active[npi] = rec.get("basic", {}).get("status") == "A"
    return active


class _Builder:
    """Accumulates entities and events across sources, then writes NDJSON."""

    def __init__(self, nppes_active: dict[str, bool]) -> None:
        self._entities: dict[str, dict[str, Any]] = {}
        self._events: list[dict[str, Any]] = []
        self._nppes_active = nppes_active
        self._sources: set[str] = set()

    def add_exclusion_record(self, rec: dict[str, Any]) -> None:
        name = rec.get("person_or_entity_name") or ""
        if not name:
            return
        npi = rec.get("npi")
        npi = str(npi).strip() if npi else None
        state = rec.get("state")
        source_id = rec.get("source_id", "unknown")
        self._sources.add(source_id)

        entity_id = npi if npi else f"name:{normalize_name(name)}"
        authority = _authority_for(source_id)
        excl = parse_any_date(rec.get("exclusion_date"))
        reinst = parse_any_date(rec.get("reinstatement_date"))

        self._upsert_entity(entity_id, name, state, npi)
        self._add_event(entity_id, rec, authority, state, excl, reinst, source_id)

    def _upsert_entity(
        self, entity_id: str, name: str, state: str | None, npi: str | None
    ) -> None:
        entity = self._entities.get(entity_id)
        if entity is None:
            entity = {
                "kind": "entity",
                "entity_id": entity_id,
                "entity_type": _entity_type(name),
                "canonical_name": name,
                "canonical_state": state,
                "npis": [],
                "npi_active": None,
                "resolution_confidence": 1.0 if npi else 0.8,
            }
            self._entities[entity_id] = entity
        if state and not entity["canonical_state"]:
            entity["canonical_state"] = state
        if npi and npi not in entity["npis"]:
            entity["npis"].append(npi)
            if npi in self._nppes_active:
                # True wins (any active NPI marks the entity active).
                entity["npi_active"] = entity["npi_active"] or self._nppes_active[npi]

    def _add_event(
        self,
        entity_id: str,
        rec: dict[str, Any],
        authority: str,
        state: str | None,
        excl: datetime | None,
        reinst: datetime | None,
        source_id: str,
    ) -> None:
        if excl is None and reinst is None:
            return
        if rec.get("indefinite_exclusion"):
            status = "indefinite"
        elif reinst is not None:
            status = "reinstated"
        else:
            status = "active"
        record_id = str(rec.get("source_record_id") or rec.get("medicaid_provider_id") or "")
        self._events.append(
            {
                "kind": "exclusion_event",
                "event_id": _event_id(entity_id, authority, _iso(excl), record_id),
                "entity_id": entity_id,
                "authority": authority,
                "exclusion_type": rec.get("provider_type"),
                "exclusion_date": _iso(excl),
                "reinstatement_date": _iso(reinst),
                "status": status,
                "state": state,
                "source_id": source_id,
                "source_record_id": record_id,
                "source_snapshot_hash": rec.get("source_snapshot_hash", ""),
                "observed_at": rec.get("observed_at") or datetime.now(UTC).isoformat(),
            }
        )

    def write(self, out: TextIO) -> tuple[int, int]:
        meta = {
            "kind": "meta",
            "export_version": "1.0.0",
            "exported_at": datetime.now(UTC).isoformat(),
            "pipeline_version": PIPELINE_VERSION,
            "entity_count": len(self._entities),
            "exclusion_event_count": len(self._events),
            "sources_ingested": sorted(self._sources),
            "jurisdiction_coverage": sorted(
                {e["canonical_state"] for e in self._entities.values() if e["canonical_state"]}
            ),
        }
        out.write(json.dumps(meta) + "\n")
        for entity in self._entities.values():
            out.write(json.dumps(entity) + "\n")
        for event in self._events:
            out.write(json.dumps(event) + "\n")
        return len(self._entities), len(self._events)


def _iter_ndjson(path: pathlib.Path) -> Any:
    if not path.exists():
        return
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                yield json.loads(line)


def build_graph(normalized_dir: pathlib.Path, output_path: pathlib.Path) -> tuple[int, int]:
    """Build the typed-NDJSON Trust Graph export. Returns (entities, events)."""
    nppes_active = _load_nppes_active(normalized_dir / "nppes_providers.ndjson")
    builder = _Builder(nppes_active)

    exclusion_sources = [
        "leie_normalized.ndjson",
        "leie_supplements_normalized.ndjson",
        "hawaii_medquest_exclusions.ndjson",
    ]
    for filename in exclusion_sources:
        for rec in _iter_ndjson(normalized_dir / filename):
            builder.add_exclusion_record(rec)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as out:
        return builder.write(out)
