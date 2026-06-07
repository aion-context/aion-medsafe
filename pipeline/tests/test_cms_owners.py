"""Tests for the CMS PECOS All-Owners normalizer (synthetic rows — no real PII)."""

from __future__ import annotations

import io

from aion_medsafe_pipeline import cms_owners as co


def _row(**kw) -> list[str]:
    row = [""] * 40
    for name, value in kw.items():
        row[co.COL[name]] = value
    return row


def test_individual_owner_name():
    row = _row(owner_type="I", owner_first="JANE", owner_middle="Q", owner_last="DOE")
    rec = co._normalize_row(row, "SNF", "h", "t")
    assert rec["owner_name"] == "DOE, JANE Q"
    assert rec["owner_type"] == "I"
    assert rec["provider_type"] == "SNF"


def test_org_owner_name_and_pct():
    row = _row(
        owner_type="O",
        owner_org="Acme Holdings LLC",
        provider_org="Sunset SNF",
        owner_state="hi",
        owner_pct="55.5",
    )
    rec = co._normalize_row(row, "SNF", "h", "t")
    assert rec["owner_name"] == "ACME HOLDINGS LLC"
    assert rec["provider_org_name"] == "SUNSET SNF"
    assert rec["owner_state"] == "HI"
    assert rec["ownership_pct"] == 55.5


def test_owner_without_name_is_dropped():
    row = _row(owner_type="I")  # no last name
    assert co._normalize_row(row, "SNF", "h", "t") is None


def test_non_numeric_pct_is_none():
    row = _row(owner_type="O", owner_org="X", owner_pct="N/A")
    assert co._normalize_row(row, "HHA", "h", "t")["ownership_pct"] is None


def test_process_bytes_skips_header_and_keeps_named_owners():
    import csv

    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(["ENROLLMENT ID"] + [""] * 39)  # header (skipped)
    w.writerow(_row(owner_type="O", owner_org="Acme LLC", provider_org="P1"))
    w.writerow(_row(owner_type="I"))  # no name -> dropped
    out = io.StringIO()
    written = co._process_bytes(buf.getvalue().encode(), "SNF", "snap", out, "t", None)
    assert written == 1
    assert "ACME LLC" in out.getvalue()
    assert "cms_pecos_owners" in out.getvalue()
