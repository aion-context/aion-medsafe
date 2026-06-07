"""Bulk NPPES acquisition + normalization (rawest source, bulk-first).

Replaces the per-NPI NPPES API with the full CMS bulk dissemination file, giving
status for ALL ~9.2M NPIs in one download instead of 8.6k rate-limited calls.

Adapted from the npi-verify proof-of-concept tooling — chiefly the NPPES CSV
field map (which ~12 of 330 columns matter) and the ACTIVE/DEACTIVATED status
rule, ported to stdlib (no requests/tqdm) and to our normalized-NDJSON contract.

Flow:
  1. download_bulk()  -> fetch monthly dissemination + deactivation ZIPs
  2. process_bulk()   -> stream npidata CSV -> normalized full-table NDJSON

Provenance: seal the raw downloaded ZIP with `aion-medsafe ingest` (BLAKE3 +
signature); the normalized table is derived from that sealed source.
"""

from __future__ import annotations

import csv
import hashlib
import json
import pathlib
import re
import sys
import zipfile
from datetime import UTC, datetime
from typing import Any
from urllib.request import Request, urlopen

BASE_URL = "https://download.cms.gov/nppes"
NPI_FILES_PAGE = f"{BASE_URL}/NPI_Files.html"
USER_AGENT = "aion-medsafe-pipeline/0.2 (Medicaid integrity)"

# Critical column indices in the 330-column NPPES dissemination CSV
# (from npi-verify/tools/process_npi_data.py).
FIELD_INDICES = {
    "npi": 0,
    "entity_type": 1,
    "replacement_npi": 2,
    "org_name": 4,
    "last_name": 5,
    "first_name": 6,
    "practice_state": 31,
    "enumeration_date": 36,
    "last_update": 37,
    "deactivation_reason": 38,
    "deactivation_date": 39,
    "reactivation_date": 40,
}

OUTPUT_NAME = "nppes_providers.ndjson"


# ─── Download ─────────────────────────────────────────────────────────────────


def get_latest_urls() -> dict[str, str | None]:
    """Scrape the CMS NPI Files page for the latest monthly + deactivation ZIPs."""
    req = Request(NPI_FILES_PAGE, headers={"User-Agent": USER_AGENT})
    with urlopen(req, timeout=30) as resp:
        html = resp.read().decode("utf-8", "replace")

    monthly: str | None = None
    deactivation: str | None = None
    for href in re.findall(r'href=["\']?([^"\' >]+\.zip)', html, re.I):
        name = href.split("/")[-1]
        lower = name.lower()
        url = href if href.startswith("http") else f"{BASE_URL}/{name}"
        if "dissemination" in lower and "weekly" not in lower:
            monthly = monthly or url
        elif "deactivat" in lower:
            deactivation = deactivation or url
    return {"monthly": monthly, "deactivation": deactivation}


def download(url: str, dest: pathlib.Path, chunk: int = 1 << 20) -> str:
    """Stream a file to `dest` (skips if present), returning its SHA-256."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    if not dest.exists():
        req = Request(url, headers={"User-Agent": USER_AGENT})
        with urlopen(req, timeout=60) as resp, open(dest, "wb") as out:
            total = int(resp.headers.get("content-length", 0))
            done = 0
            while True:
                buf = resp.read(chunk)
                if not buf:
                    break
                out.write(buf)
                done += len(buf)
                if total:
                    pct = done * 100 // total
                    print(f"\r  {dest.name}: {pct}% ({done >> 20} MiB)", end="", file=sys.stderr)
        print("", file=sys.stderr)
    return _sha256(dest)


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def download_bulk(
    data_dir: pathlib.Path,
    monthly_url: str | None = None,
    deactivation_url: str | None = None,
) -> dict[str, str]:
    """Download the monthly dissemination + deactivation ZIPs. Returns
    {path: sha256} for each file fetched."""
    if monthly_url is None or deactivation_url is None:
        latest = get_latest_urls()
        monthly_url = monthly_url or latest["monthly"]
        deactivation_url = deactivation_url or latest["deactivation"]
    if not monthly_url:
        raise RuntimeError(f"could not resolve monthly NPPES URL; see {NPI_FILES_PAGE}")

    raw_dir = data_dir / "raw" / "nppes_bulk"
    result: dict[str, str] = {}
    monthly_path = raw_dir / monthly_url.split("/")[-1]
    result[str(monthly_path)] = download(monthly_url, monthly_path)
    if deactivation_url:
        deact_path = raw_dir / deactivation_url.split("/")[-1]
        result[str(deact_path)] = download(deactivation_url, deact_path)
    return result


def extract_main_csv(zip_path: pathlib.Path, dest_dir: pathlib.Path) -> pathlib.Path:
    """Extract the main npidata_pfile CSV (not the _fileheader) from the ZIP."""
    with zipfile.ZipFile(zip_path) as zf:
        for name in zf.namelist():
            base = name.split("/")[-1]
            if base.startswith("npidata_pfile_") and base.endswith(".csv") and "fileheader" not in base.lower():
                zf.extract(name, dest_dir)
                return dest_dir / name
    raise RuntimeError(f"no npidata_pfile CSV found in {zip_path.name}")


# ─── Process ──────────────────────────────────────────────────────────────────


def _status(deactivation_date: str, reactivation_date: str) -> str:
    if reactivation_date:
        return "ACTIVE"
    return "DEACTIVATED" if deactivation_date else "ACTIVE"


def _normalize_row(row: list[str], snapshot_hash: str, observed_at: str) -> dict[str, Any] | None:
    if len(row) < 50:
        return None
    npi = row[FIELD_INDICES["npi"]].strip()
    if len(npi) != 10 or not npi.isdigit():
        return None
    entity_type = row[FIELD_INDICES["entity_type"]].strip()
    if entity_type == "2":
        name = row[FIELD_INDICES["org_name"]].strip()
    else:
        last = row[FIELD_INDICES["last_name"]].strip()
        first = row[FIELD_INDICES["first_name"]].strip()
        name = f"{last}, {first}".strip(", ") if (last or first) else ""
    deactivation_date = row[FIELD_INDICES["deactivation_date"]].strip()
    reactivation_date = row[FIELD_INDICES["reactivation_date"]].strip()
    return {
        "npi": npi,
        "status": _status(deactivation_date, reactivation_date),
        "entity_type": int(entity_type) if entity_type.isdigit() else 1,
        "name": name or None,
        "state": row[FIELD_INDICES["practice_state"]].strip() or None,
        "enumeration_date": row[FIELD_INDICES["enumeration_date"]].strip() or None,
        "last_update": row[FIELD_INDICES["last_update"]].strip() or None,
        "deactivation_reason": row[FIELD_INDICES["deactivation_reason"]].strip() or None,
        "deactivation_date": deactivation_date or None,
        "reactivation_date": reactivation_date or None,
        "source_id": "cms_nppes_bulk",
        "source_snapshot_hash": snapshot_hash,
    }


def _process_reader(reader, out_path: pathlib.Path, snapshot_hash: str, limit: int | None) -> dict[str, int]:
    """Normalize CSV rows from any reader into the full-table NDJSON."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    stats = {"rows": 0, "written": 0, "active": 0, "deactivated": 0, "skipped": 0}
    observed_at = datetime.now(UTC).isoformat()
    with open(out_path, "w", encoding="utf-8") as out:
        next(reader, None)  # header
        for row in reader:
            stats["rows"] += 1
            record = _normalize_row(row, snapshot_hash, observed_at)
            if record is None:
                stats["skipped"] += 1
                continue
            out.write(json.dumps(record) + "\n")
            stats["written"] += 1
            stats["active" if record["status"] == "ACTIVE" else "deactivated"] += 1
            if limit is not None and stats["written"] >= limit:
                break
    return stats


def process_bulk(
    csv_path: pathlib.Path,
    out_path: pathlib.Path,
    snapshot_hash: str,
    limit: int | None = None,
) -> dict[str, int]:
    """Normalize an extracted npidata CSV into the full-table NDJSON."""
    with open(csv_path, encoding="utf-8", errors="replace", newline="") as src:
        return _process_reader(csv.reader(src), out_path, snapshot_hash, limit)


def _main_csv_name(zf: zipfile.ZipFile) -> str:
    for name in zf.namelist():
        base = name.split("/")[-1]
        if base.startswith("npidata_pfile_") and base.endswith(".csv") and "fileheader" not in base.lower():
            return name
    raise RuntimeError("no npidata_pfile CSV in archive")


def process_bulk_zip(
    zip_path: pathlib.Path,
    out_path: pathlib.Path,
    snapshot_hash: str,
    limit: int | None = None,
) -> dict[str, int]:
    """Normalize the npidata CSV by STREAMING it out of the ZIP — never extracts
    the ~9 GB uncompressed file to disk."""
    import io

    with zipfile.ZipFile(zip_path) as zf:
        with zf.open(_main_csv_name(zf)) as raw:
            text = io.TextIOWrapper(raw, encoding="utf-8", errors="replace", newline="")
            return _process_reader(csv.reader(text), out_path, snapshot_hash, limit)
