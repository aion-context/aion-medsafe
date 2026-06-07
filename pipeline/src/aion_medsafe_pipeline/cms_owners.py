"""CMS PECOS "All Owners" ownership data — acquisition + normalization.

CMS publishes monthly bulk CSVs of Medicare provider OWNERSHIP (from PECOS) for
six provider types — Skilled Nursing Facility, Home Health Agency, Hospice,
Hospital, FQHC, Rural Health Clinic — openly on data.cms.gov, no account. This
is the real ownership-network source (vs. our shared address/phone approximation).

We DISCOVER the latest dated CSV URLs from the data.cms.gov DKAN metastore
(data.json) rather than hard-coding month-stamped paths, then normalize every
(provider, owner) edge into one NDJSON. The downstream Rust correlation matches
OWNERS against the excluded universe by name+state — i.e. "is an excluded party
an owner of active Medicare providers?".

Note: these files key the OWNED provider by PECOS enrollment + organization name
(no NPI), so owner→excluded matching is name-based, not NPI-based.
"""

from __future__ import annotations

import csv
import hashlib
import io
import json
import pathlib
import re
import sys
from datetime import UTC, datetime
from typing import Any
from urllib.request import Request, urlopen

METASTORE_URL = "https://data.cms.gov/data.json"
USER_AGENT = "aion-medsafe-pipeline/0.2 (Medicaid integrity)"
OUTPUT_NAME = "cms_owners.ndjson"
SOURCE_ID = "cms_pecos_owners"

# Dataset title -> short provider-type label.
TITLE_TO_TYPE = {
    "skilled nursing facility all owners": "SNF",
    "home health agency all owners": "HHA",
    "hospice all owners": "HOSPICE",
    "hospital all owners": "HOSPITAL",
    "federally qualified health center all owners": "FQHC",
    "rural health clinic all owners": "RHC",
}

# 0-indexed columns in the All-Owners layout (consistent across the six files).
COL = {
    "provider_org": 2,
    "provider_pac_id": 1,
    "provider_enrollment_id": 0,
    "owner_pac_id": 3,
    "owner_type": 4,
    "owner_role": 6,
    "owner_first": 8,
    "owner_middle": 9,
    "owner_last": 10,
    "owner_org": 12,
    "owner_state": 17,
    "owner_pct": 19,
}


def discover() -> dict[str, str]:
    """Resolve {provider_type: latest CSV URL} from the CMS metastore."""
    req = Request(METASTORE_URL, headers={"User-Agent": USER_AGENT})
    catalog = json.load(urlopen(req, timeout=60))
    out: dict[str, str] = {}
    for ds in catalog.get("dataset", []):
        ptype = TITLE_TO_TYPE.get(ds.get("title", "").strip().lower())
        if not ptype:
            continue
        for dist in ds.get("distribution", []):
            url = dist.get("downloadURL", "")
            if url.lower().endswith(".csv"):
                out[ptype] = url
                break
    return out


def _sha256_stream(resp, out_file, chunk: int = 1 << 20) -> tuple[str, bytes]:
    """Tee a response to disk while hashing; return (sha256, full bytes)."""
    digest = hashlib.sha256()
    buf = io.BytesIO()
    while True:
        block = resp.read(chunk)
        if not block:
            break
        digest.update(block)
        buf.write(block)
        out_file.write(block)
    return digest.hexdigest(), buf.getvalue()


def _owner_name(row: list[str]) -> str | None:
    if row[COL["owner_type"]].strip().upper() == "I":
        last = row[COL["owner_last"]].strip()
        first = " ".join(p for p in (row[COL["owner_first"]].strip(), row[COL["owner_middle"]].strip()) if p)
        name = f"{last}, {first}".strip().strip(",").strip()
        return name.upper() or None
    return (row[COL["owner_org"]].strip() or None) and row[COL["owner_org"]].strip().upper()


def _normalize_row(row: list[str], ptype: str, snapshot_hash: str, observed_at: str) -> dict[str, Any] | None:
    if len(row) <= COL["owner_pct"]:
        return None
    owner_name = _owner_name(row)
    if not owner_name:
        return None
    pct = row[COL["owner_pct"]].strip()
    return {
        "provider_org_name": row[COL["provider_org"]].strip().upper() or None,
        "provider_pac_id": row[COL["provider_pac_id"]].strip() or None,
        "provider_enrollment_id": row[COL["provider_enrollment_id"]].strip() or None,
        "provider_type": ptype,
        "owner_type": row[COL["owner_type"]].strip().upper() or None,
        "owner_name": owner_name,
        "owner_state": row[COL["owner_state"]].strip().upper() or None,
        "owner_role": row[COL["owner_role"]].strip() or None,
        "ownership_pct": float(pct) if _is_float(pct) else None,
        "owner_pac_id": row[COL["owner_pac_id"]].strip() or None,
        "source_id": SOURCE_ID,
        "source_snapshot_hash": snapshot_hash,
        "observed_at": observed_at,
    }


def _is_float(s: str) -> bool:
    try:
        float(s)
        return True
    except ValueError:
        return False


def _process_bytes(data: bytes, ptype: str, snapshot_hash: str, out, observed_at: str, limit: int | None) -> int:
    reader = csv.reader(io.TextIOWrapper(io.BytesIO(data), encoding="utf-8", errors="replace", newline=""))
    next(reader, None)  # header
    written = 0
    for row in reader:
        rec = _normalize_row(row, ptype, snapshot_hash, observed_at)
        if rec is None:
            continue
        out.write(json.dumps(rec) + "\n")
        written += 1
        if limit is not None and written >= limit:
            break
    return written


def run(data_dir: pathlib.Path, limit: int | None = None) -> dict[str, Any]:
    """Discover + download + normalize all six All-Owners files into one NDJSON."""
    urls = discover()
    if not urls:
        raise RuntimeError(f"no All-Owners datasets found in {METASTORE_URL}")
    raw_dir = data_dir / "raw" / "cms_owners"
    raw_dir.mkdir(parents=True, exist_ok=True)
    out_path = data_dir / "normalized" / OUTPUT_NAME
    out_path.parent.mkdir(parents=True, exist_ok=True)
    observed_at = datetime.now(UTC).isoformat()
    per_type: dict[str, int] = {}
    hashes: dict[str, str] = {}
    with open(out_path, "w", encoding="utf-8") as out:
        for ptype, url in sorted(urls.items()):
            dest = raw_dir / url.split("/")[-1]
            print(f"  {ptype}: downloading {dest.name}", file=sys.stderr)
            req = Request(url, headers={"User-Agent": USER_AGENT})
            with urlopen(req, timeout=300) as resp, open(dest, "wb") as raw_out:
                sha, data = _sha256_stream(resp, raw_out)
            hashes[ptype] = sha
            per_type[ptype] = _process_bytes(data, ptype, sha, out, observed_at, limit)
    return {"out": out_path, "per_type": per_type, "hashes": hashes, "total": sum(per_type.values())}
