"""Tests for the Hawaii DCCA disciplinary-actions parser (synthetic HTML)."""

from __future__ import annotations

from aion_medsafe_pipeline import hawaii_licensing as hl

SAMPLE = """
<h2>BOARD OF NURSING</h2>
<p>Respondent: Jane Q. Doe fka Jane Smith</p>
<p>Case Number: RNS 2025-01-L</p>
<p>Sanction: License revoked</p>
<p>Effective Date: 8-14-25</p>
<p>RICO alleges ...</p>

<h2>BOARD OF NURSING</h2>
<p>Respondent: Minor Penalty Person</p>
<p>Sanction: $500 fine, continuing education</p>
<p>Effective Date: 7-01-25</p>

<h2>BOARD OF PUBLIC ACCOUNTANCY</h2>
<p>Respondent: Some CPA LLP</p>
<p>Sanction: Permit suspended</p>
<p>Effective Date: 6-01-25</p>
"""


def test_keeps_healthcare_serious_only():
    records, stats = hl.parse(SAMPLE, "snap", "t")
    # Only the revoked nurse is kept: fine -> minor; CPA -> non-healthcare.
    assert len(records) == 1
    r = records[0]
    assert r["person_or_entity_name"] == "JANE Q. DOE"  # alias stripped
    assert r["state"] == "HI"
    assert r["source_id"] == "hawaii_dcca_license"
    assert r["indefinite_exclusion"] is True  # revocation
    assert r["exclusion_date"] == "2025-08-14T00:00:00+00:00"
    assert stats["skip_minor_sanction"] == 1
    assert stats["skip_non_healthcare"] == 1


def test_date_parsing_variants():
    assert hl._iso_date("8-14-25").startswith("2025-08-14")
    assert hl._iso_date("12/01/2024").startswith("2024-12-01")
    assert hl._iso_date("no date here") is None


def test_heading_detection():
    assert hl._is_heading("BOARD OF NURSING")
    assert not hl._is_heading("Respondent: Jane Doe")
    assert not hl._is_heading("RICO alleges something")
