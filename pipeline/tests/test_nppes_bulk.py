"""Tests for bulk NPPES normalization (field map + status rule)."""

from __future__ import annotations

import csv
import json
import pathlib

from aion_medsafe_pipeline.nppes_bulk import FIELD_INDICES, _status, process_bulk


def _row(npi, etype, last="", first="", org="", state="HI", deact="", react=""):
    """Build a synthetic NPPES dissemination row (51 cols, fake NPIs only)."""
    row = [""] * 51
    row[FIELD_INDICES["npi"]] = npi
    row[FIELD_INDICES["entity_type"]] = etype
    row[FIELD_INDICES["org_name"]] = org
    row[FIELD_INDICES["last_name"]] = last
    row[FIELD_INDICES["first_name"]] = first
    row[FIELD_INDICES["practice_state"]] = state
    row[FIELD_INDICES["deactivation_date"]] = deact
    row[FIELD_INDICES["reactivation_date"]] = react
    return row


def test_status_rule() -> None:
    assert _status("", "") == "ACTIVE"
    assert _status("01/01/2020", "") == "DEACTIVATED"
    # A reactivation overrides a prior deactivation.
    assert _status("01/01/2020", "06/01/2021") == "ACTIVE"


def test_process_bulk_normalizes_fields_and_status(tmp_path: pathlib.Path) -> None:
    csv_path = tmp_path / "npidata.csv"
    with open(csv_path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow([f"col{i}" for i in range(51)])  # header
        w.writerow(_row("1234567890", "1", last="DOE", first="JANE", state="HI"))
        w.writerow(_row("1999999999", "2", org="ACME HOME HEALTH LLC", state="CA", deact="01/01/2020"))
        w.writerow(_row("bad", "1"))  # invalid NPI -> skipped

    out = tmp_path / "nppes_providers.ndjson"
    stats = process_bulk(csv_path, out, "snap123")

    assert stats["written"] == 2
    assert stats["active"] == 1
    assert stats["deactivated"] == 1
    assert stats["skipped"] == 1

    rows = {json.loads(line)["npi"]: json.loads(line) for line in out.read_text().splitlines()}
    assert rows["1234567890"]["status"] == "ACTIVE"
    assert rows["1234567890"]["name"] == "DOE, JANE"
    assert rows["1234567890"]["source_snapshot_hash"] == "snap123"
    assert rows["1999999999"]["status"] == "DEACTIVATED"
    assert rows["1999999999"]["entity_type"] == 2
    assert rows["1999999999"]["name"] == "ACME HOME HEALTH LLC"


def test_process_bulk_respects_limit(tmp_path: pathlib.Path) -> None:
    csv_path = tmp_path / "npidata.csv"
    with open(csv_path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow([f"col{i}" for i in range(51)])
        for n in range(5):
            w.writerow(_row(f"100000000{n}", "1", last=f"P{n}", first="X"))
    out = tmp_path / "out.ndjson"
    stats = process_bulk(csv_path, out, "snap", limit=2)
    assert stats["written"] == 2
