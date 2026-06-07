"""Tests for the Trust Graph export (build-graph)."""

from __future__ import annotations

import json
import pathlib

from aion_medsafe_pipeline.graph_export import (
    _authority_for,
    _entity_type,
    build_graph,
    parse_any_date,
)


def test_parse_any_date_iso_and_us_short() -> None:
    iso = parse_any_date("2020-03-19T00:00:00Z")
    assert iso is not None and iso.year == 2020 and iso.month == 3

    us = parse_any_date("04/20/15")
    assert us is not None and us.year == 2015 and us.month == 4 and us.day == 20

    assert parse_any_date(None) is None
    assert parse_any_date("") is None
    assert parse_any_date("not-a-date") is None


def test_entity_type_heuristic() -> None:
    assert _entity_type("ACHAVAL MARIA") == "individual"
    assert _entity_type("#1 MARKETING SERVICE, INC") == "organization"
    assert _entity_type("ALOHA HOME HEALTH LLC") == "organization"


def test_authority_mapping() -> None:
    assert _authority_for("hhs_oig_leie") == "hhs_oig"
    assert _authority_for("hhs_oig_leie_supplement") == "hhs_oig"
    assert _authority_for("hawaii_medquest_exclusions") == "state_medicaid"
    assert _authority_for("unknown_source") == "hhs_oig"


def _write_ndjson(path: pathlib.Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")


def test_build_graph_emits_typed_ndjson(tmp_path: pathlib.Path) -> None:
    norm = tmp_path / "normalized"
    _write_ndjson(
        norm / "leie_normalized.ndjson",
        [
            {
                "source_id": "hhs_oig_leie",
                "source_record_id": "0",
                "observed_at": "2026-06-06T00:00:00Z",
                "person_or_entity_name": "DOE JANE",
                "npi": "1234567890",
                "exclusion_date": "2015-01-01T00:00:00Z",
                "reinstatement_date": None,
                "state": "HI",
                "source_snapshot_hash": "h1",
            }
        ],
    )
    _write_ndjson(
        norm / "hawaii_medquest_exclusions.ndjson",
        [
            {
                "source_id": "hawaii_medquest_exclusions",
                "person_or_entity_name": "DOE JANE",
                "medicaid_provider_id": "999",
                "exclusion_date": "04/20/15",
                "reinstatement_date": "5/18/20",
                "state": "HI",
                "source_snapshot_hash": "h2",
            }
        ],
    )

    out = tmp_path / "trust_graph.ndjson"
    entities, events = build_graph(norm, out)

    # DOE JANE appears in both sources under the same NPI-less/name key path;
    # the federal row carries an NPI so resolves to that entity_id.
    lines = [json.loads(line) for line in out.read_text().splitlines()]
    kinds = [line["kind"] for line in lines]
    assert kinds[0] == "meta"
    assert "entity" in kinds
    assert "exclusion_event" in kinds
    assert lines[0]["entity_count"] == entities
    assert lines[0]["exclusion_event_count"] == events

    authorities = {
        line["authority"] for line in lines if line["kind"] == "exclusion_event"
    }
    assert "hhs_oig" in authorities
    assert "state_medicaid" in authorities
