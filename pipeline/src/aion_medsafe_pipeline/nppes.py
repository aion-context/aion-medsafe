"""NPPES NPI-status enrichment — acquisition for `active_npi_while_excluded`.

Fetches records from the public NPPES registry for excluded providers so the
Rust system can flag NPIs that remain ACTIVE despite an exclusion. Designed for
full coverage of the ~8.6k excluded NPIs:

- pulls NPIs from every exclusion source (LEIE + supplements)
- optional state prioritization (e.g. fetch the Hawaii-nexus set first)
- resumable: skips NPIs already present in the output (safe to re-run)
- rate-limited and appends incrementally (partial progress always persists)

Output rows are the raw NPPES API result plus `_source_id`/`_fetched_at`/
`_snapshot_hash`; the Rust `build-graph` reads `number` + `basic.status`.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import time
from datetime import UTC, datetime
from typing import Any
from urllib.request import Request, urlopen

NPPES_API = "https://npiregistry.cms.hhs.gov/api/?version=2.1&number={npi}"
EXCLUSION_FILES = ["leie_normalized.ndjson", "leie_supplements_normalized.ndjson"]
OUTPUT_NAME = "nppes_providers.ndjson"


def excluded_npis(normalized_dir: pathlib.Path, state: str | None = None) -> list[str]:
    """Unique NPIs across exclusion sources, in first-seen order. When `state`
    is set, only records with that state nexus are included."""
    seen: set[str] = set()
    ordered: list[str] = []
    for filename in EXCLUSION_FILES:
        path = normalized_dir / filename
        if not path.exists():
            continue
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                rec = json.loads(line)
                npi = rec.get("npi")
                if not npi:
                    continue
                if state and rec.get("state") != state:
                    continue
                npi = str(npi)
                if npi not in seen:
                    seen.add(npi)
                    ordered.append(npi)
    return ordered


def already_fetched(output_path: pathlib.Path) -> set[str]:
    done: set[str] = set()
    if not output_path.exists():
        return done
    with open(output_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            number = json.loads(line).get("number")
            if number:
                done.add(str(number))
    return done


def _fetch_one(npi: str, fetched_at: str) -> dict[str, Any] | None:
    req = Request(NPPES_API.format(npi=npi), headers={"User-Agent": "aion-medsafe-pipeline/0.2"})
    with urlopen(req, timeout=20) as resp:
        raw = resp.read()
    data = json.loads(raw)
    if data.get("result_count", 0) <= 0:
        return None
    result = data["results"][0]
    result["_source_id"] = "cms_nppes_npi_registry"
    result["_fetched_at"] = fetched_at
    result["_snapshot_hash"] = hashlib.sha256(raw).hexdigest()
    return result


def fetch_nppes(
    normalized_dir: pathlib.Path,
    state: str | None = None,
    limit: int | None = None,
    sleep_s: float = 0.05,
) -> dict[str, int]:
    """Fetch NPPES records for excluded NPIs (resumable). Returns counts."""
    output_path = normalized_dir / OUTPUT_NAME
    output_path.parent.mkdir(parents=True, exist_ok=True)

    targets = excluded_npis(normalized_dir, state)
    done = already_fetched(output_path)
    todo = [npi for npi in targets if npi not in done]
    if limit is not None:
        todo = todo[:limit]

    stats = {
        "targets": len(targets),
        "already_fetched": len(done),
        "attempted": len(todo),
        "fetched": 0,
        "active": 0,
        "errors": 0,
    }

    with open(output_path, "a", encoding="utf-8") as out:
        fetched_at = datetime.now(UTC).isoformat()
        for npi in todo:
            try:
                result = _fetch_one(npi, fetched_at)
                if result is not None:
                    out.write(json.dumps(result) + "\n")
                    out.flush()
                    stats["fetched"] += 1
                    if result.get("basic", {}).get("status") == "A":
                        stats["active"] += 1
            except Exception:
                stats["errors"] += 1
            if sleep_s:
                time.sleep(sleep_s)
    return stats
