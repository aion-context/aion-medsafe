"""Tests for the SAM.gov exclusions normalizer (synthetic rows — no real PII)."""

from __future__ import annotations

import json

from aion_medsafe_pipeline import sam_exclusions as sam


def _row(**kw) -> list[str]:
    """Build a 29-column SAM extract row, overriding by COL name."""
    row = [""] * 29
    for name, value in kw.items():
        row[sam.COL[name]] = value
    return row


def test_individual_name_formatting():
    row = _row(classification="Individual", first="JOHN", middle="Q", last="DOE", npi="1234567890")
    rec = sam._normalize_row(row, "hash", "t")
    assert rec is not None
    assert rec["person_or_entity_name"] == "DOE, JOHN Q"
    assert rec["npi"] == "1234567890"


def test_firm_name_and_termination_date_is_reinstatement():
    row = _row(
        classification="Firm",
        name="Acme Labs LLC",
        state="ca",
        active_date="02/02/2019",
        termination_date="03/03/2025",
        npi="9999999999",
    )
    rec = sam._normalize_row(row, "h", "t")
    assert rec["person_or_entity_name"] == "ACME LABS LLC"
    assert rec["state"] == "CA"
    assert rec["exclusion_date"] == "02/02/2019"
    assert rec["reinstatement_date"] == "03/03/2025"
    assert rec["indefinite_exclusion"] is False


def test_indefinite_termination():
    row = _row(classification="Individual", last="ROE", npi="1112223334", termination_date="Indefinite")
    rec = sam._normalize_row(row, "h", "t")
    assert rec["indefinite_exclusion"] is True
    assert rec["reinstatement_date"] is None


def test_npi_extracted_from_text_field():
    row = _row(classification="Firm", name="X", npi="NPI 1112223334 primary")
    rec = sam._normalize_row(row, "h", "t")
    assert rec["npi"] == "1112223334"


def test_provider_type_combines_exclusion_type_and_agency():
    row = _row(
        classification="Firm",
        name="X",
        npi="1234567890",
        exclusion_type="Healthcare Fraud",
        excluding_agency="HHS",
    )
    rec = sam._normalize_row(row, "h", "t")
    assert rec["provider_type"] == "Healthcare Fraud — HHS"


def test_no_npi_is_dropped():
    row = _row(classification="Firm", name="Non-Healthcare Contractor")
    assert sam._normalize_row(row, "h", "t") is None


def test_process_writes_only_npi_rows(tmp_path):
    import csv

    csv_path = tmp_path / "sam.csv"
    with open(csv_path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["Classification"] + [""] * 28)  # header (skipped)
        w.writerow(_row(classification="Individual", last="DOE", npi="1234567890"))
        w.writerow(_row(classification="Firm", name="No NPI Contractor"))  # dropped
    out = tmp_path / "out.ndjson"
    stats = sam.process(csv_path, out, "snap")
    assert stats["written"] == 1
    assert stats["dropped_no_npi"] == 1
    lines = out.read_text().strip().splitlines()
    assert len(lines) == 1
    rec = json.loads(lines[0])
    assert rec["source_id"] == "sam_gov"
    assert rec["source_snapshot_hash"] == "snap"
