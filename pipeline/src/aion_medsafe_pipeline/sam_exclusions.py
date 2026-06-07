"""SAM.gov Exclusions Public Extract — acquisition + normalization (bulk-first).

The federal procurement exclusion list (System for Award Management). The
key-gated thing is the *API* and the *sensitive* extract; the **Public** extract
is openly mirrored on data.gov (no account), which we use here per ADR-001.

We keep only the HEALTHCARE-relevant subset: records carrying a valid 10-digit
NPI. NPI is the reliable cross-reference into our LEIE/NPPES universe; name-only
matching to non-healthcare procurement debarments would manufacture false links,
so those rows are dropped (and counted). Output conforms to the same normalized
exclusion NDJSON contract as LEIE, so it flows straight into the Rust graph
(source_id `sam_gov` -> ExclusionAuthority::SamGov).

Layout: SAM Exclusions Public Extract V2.1 (29 columns, documented order).
"""

from __future__ import annotations

import csv
import hashlib
import json
import pathlib
import re
import sys
from datetime import UTC, datetime
from typing import Any
from urllib.request import Request, urlopen

# data.gov public-extract resource (302-redirects to the us-gov S3 file).
SAM_PUBLIC_EXTRACT_URL = (
    "https://inventory.data.gov/dataset/"
    "7416a2e4-9aa7-4bcd-801c-20f25a545916/resource/"
    "78bb6c57-42e8-4055-931d-928ebcbde39f/download/"
    "samexclusionspublicextract-gsa-1626.csv"
)
USER_AGENT = "aion-medsafe-pipeline/0.2 (Medicaid integrity)"
OUTPUT_NAME = "sam_exclusions.ndjson"
SOURCE_ID = "sam_gov"

# 0-indexed column order from SAM_Exclusions_Public_Extract_Layout V2.1.
COL = {
    "classification": 0,
    "name": 1,
    "first": 3,
    "middle": 4,
    "last": 6,
    "state": 12,
    "country": 13,
    "exclusion_program": 16,
    "excluding_agency": 17,
    "exclusion_type": 19,
    "active_date": 21,
    "termination_date": 22,
    "sam_number": 25,
    "npi": 27,
}
_NPI_RE = re.compile(r"\b\d{10}\b")
_DATE_RE = re.compile(r"^\d{1,2}/\d{1,2}/\d{4}$")


def download(url: str, dest: pathlib.Path, chunk: int = 1 << 20) -> str:
    """Stream the extract to `dest` (skips if present), returning its SHA-256.
    urllib follows the data.gov -> S3 redirect automatically."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    if not dest.exists():
        req = Request(url, headers={"User-Agent": USER_AGENT})
        with urlopen(req, timeout=120) as resp, open(dest, "wb") as out:
            total = int(resp.headers.get("content-length", 0))
            done = 0
            while True:
                buf = resp.read(chunk)
                if not buf:
                    break
                out.write(buf)
                done += len(buf)
                if total:
                    print(f"\r  {dest.name}: {done * 100 // total}%", end="", file=sys.stderr)
        print("", file=sys.stderr)
    return _sha256(dest)


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def _npi(field: str) -> str | None:
    match = _NPI_RE.search(field or "")
    return match.group(0) if match else None


def _name(row: list[str]) -> str:
    """Individuals -> 'LAST, FIRST MIDDLE'; firms/vessels/entities -> Name."""
    if row[COL["classification"]].strip().lower() == "individual":
        last = row[COL["last"]].strip()
        first = " ".join(p for p in (row[COL["first"]].strip(), row[COL["middle"]].strip()) if p)
        name = f"{last}, {first}".strip().strip(",").strip()
        return name.upper()
    return row[COL["name"]].strip().upper()


def _normalize_row(row: list[str], snapshot_hash: str, observed_at: str) -> dict[str, Any] | None:
    if len(row) <= COL["npi"]:
        return None
    npi = _npi(row[COL["npi"]])
    if npi is None:  # healthcare-relevant subset only (reliable cross-reference)
        return None
    name = _name(row)
    if not name:
        return None
    termination = row[COL["termination_date"]].strip()
    indefinite = termination.lower() == "indefinite"
    reinstatement = termination if _DATE_RE.match(termination) else None
    state = row[COL["state"]].strip().upper() or None
    return {
        "person_or_entity_name": name,
        "npi": npi,
        "state": state,
        "exclusion_date": row[COL["active_date"]].strip() or None,
        "reinstatement_date": reinstatement,
        "indefinite_exclusion": indefinite,
        "source_id": SOURCE_ID,
        "source_record_id": row[COL["sam_number"]].strip() or None,
        "provider_type": row[COL["exclusion_type"]].strip() or None,
        "source_snapshot_hash": snapshot_hash,
        "observed_at": observed_at,
    }


def process(csv_path: pathlib.Path, out_path: pathlib.Path, snapshot_hash: str, limit: int | None = None) -> dict[str, int]:
    """Normalize the SAM public extract CSV into the exclusion NDJSON contract."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    stats = {"rows": 0, "written": 0, "dropped_no_npi": 0, "with_state": 0, "indefinite": 0}
    observed_at = datetime.now(UTC).isoformat()
    with open(csv_path, encoding="utf-8", errors="replace", newline="") as src, open(
        out_path, "w", encoding="utf-8"
    ) as out:
        reader = csv.reader(src)
        first = True
        for row in reader:
            if first:
                first = False
                if row and row[0].strip().lower() == "classification":
                    continue  # skip header row if present
            stats["rows"] += 1
            record = _normalize_row(row, snapshot_hash, observed_at)
            if record is None:
                stats["dropped_no_npi"] += 1
                continue
            out.write(json.dumps(record) + "\n")
            stats["written"] += 1
            if record["state"]:
                stats["with_state"] += 1
            if record["indefinite_exclusion"]:
                stats["indefinite"] += 1
            if limit is not None and stats["written"] >= limit:
                break
    return stats


def run(data_dir: pathlib.Path, url: str = SAM_PUBLIC_EXTRACT_URL, limit: int | None = None) -> dict[str, Any]:
    """Download + normalize. Returns {csv, snapshot_hash, out, stats}."""
    raw_path = data_dir / "raw" / "bulk" / "sam_exclusions_public_extract.csv"
    snapshot_hash = download(url, raw_path)
    out_path = data_dir / "normalized" / OUTPUT_NAME
    stats = process(raw_path, out_path, snapshot_hash, limit=limit)
    return {"csv": raw_path, "snapshot_hash": snapshot_hash, "out": out_path, "stats": stats}
