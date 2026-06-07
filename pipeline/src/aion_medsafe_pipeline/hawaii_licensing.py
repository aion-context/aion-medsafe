"""Hawaii DCCA / RICO professional-license disciplinary actions — acquisition +
normalization.

The state licensing board's serious adverse actions (license revocation /
suspension / surrender) are exclusion-like: the provider can no longer lawfully
practice. DCCA publishes them on public OAH "Disciplinary Actions" release pages
(no account), but as inline HTML text blocks with no license numbers or NPIs:

    BOARD OF NURSING
    Respondent:     <name>
    Case Number:    <case>
    Sanction:       <action + penalty>
    Effective Date: <m-d-yy>
    <allegation>

This is the weakest source we ingest: matching to our universe is NAME-only and
HI-scoped (no NPI). We therefore keep only the HEALTHCARE boards and the SERIOUS
(exclusion-like) sanctions, and count everything else. Output conforms to the
normalized exclusion contract; source_id `hawaii_dcca_license` ->
ExclusionAuthority::StateLicense.
"""

from __future__ import annotations

import hashlib
import html
import json
import pathlib
import re
from datetime import UTC, datetime
from typing import Any
from urllib.request import Request, urlopen

# Disciplinary releases are published per period; we ingest a window of recent
# ones and de-duplicate (the value is the accumulated history). Override with
# --url for a single page. More can be added as DCCA publishes them.
_OAH = "https://cca.hawaii.gov/oah/"
DEFAULT_RELEASE_URLS = [
    _OAH + "release-dcca-disciplinary-actions-through-september-2025/",
    _OAH + "disciplinary-actions-july-2025/",
    _OAH + "release-dcca-disciplinary-actions-through-april-2025/",
    _OAH + "release-dcca-disciplinary-actions-through-february-2025/",
    _OAH + "release-dcca-disciplinary-actions-through-september-2024/",
    _OAH + "release-dcca-disciplinary-actions-through-september-2023/",
    _OAH + "release-dcca-disciplinary-actions-through-may-2023/",
]
USER_AGENT = "aion-medsafe-pipeline/0.2 (Medicaid integrity)"
OUTPUT_NAME = "hawaii_dcca_license.ndjson"
SOURCE_ID = "hawaii_dcca_license"

# Boards whose licensees are healthcare providers (Medicaid-relevant).
HEALTHCARE_BOARD = re.compile(
    r"NURSING|MEDICAL|MEDICINE|PHARMAC|DENTAL|DENTIST|PSYCHOLOG|SOCIAL WORK|OSTEOPATH"
    r"|CHIROPRACT|OPTOMETR|ACUPUNCT|NATUROPATH|SPEECH|PHYSICAL THERAP|OCCUPATIONAL THERAP"
    r"|MARRIAGE|MENTAL HEALTH|COUNSEL|MIDWIF|RESPIRATORY|NURSING HOME|CARE",
    re.IGNORECASE,
)
# Sanctions that bar practice (exclusion-like) — not fines / CE / reprimands.
SERIOUS_SANCTION = re.compile(
    r"revoc|revoke|suspen|surrender|relinquish|forfeit|barred|denial|denied|voluntar",
    re.IGNORECASE,
)
_DATE = re.compile(r"\d{1,2}[-/]\d{1,2}[-/]\d{2,4}")


def _to_text_lines(raw_html: str) -> list[str]:
    """Tag-stripped, line-structured text (block tags -> newlines)."""
    text = re.sub(r"<[^>]+>", "\n", raw_html)
    text = html.unescape(text)
    lines = []
    for line in text.split("\n"):
        line = re.sub(r"[ \t]+", " ", line).strip()
        if line:
            lines.append(line)
    return lines


def _is_heading(line: str) -> bool:
    """A board/commission heading: an all-caps phrase, not a field label."""
    if not line or line != line.upper():
        return False
    if line.startswith(("RESPONDENT", "CASE", "SANCTION", "EFFECTIVE", "RICO")):
        return False
    return len(line) >= 6 and len(re.findall(r"[A-Z]{2,}", line)) >= 2


def _iso_date(raw: str) -> str | None:
    m = _DATE.search(raw or "")
    if not m:
        return None
    token = m.group(0).replace("/", "-")
    for fmt in ("%m-%d-%y", "%m-%d-%Y"):
        try:
            return datetime.strptime(token, fmt).strftime("%Y-%m-%dT00:00:00+00:00")
        except ValueError:
            continue
    return None


def parse(raw_html: str, snapshot_hash: str, observed_at: str) -> tuple[list[dict[str, Any]], dict[str, int]]:
    """Parse the release page into normalized records + stats (pure/testable)."""
    lines = _to_text_lines(raw_html)
    stats = {"entries": 0, "written": 0, "skip_non_healthcare": 0, "skip_minor_sanction": 0}
    records: list[dict[str, Any]] = []
    board = None
    cur: dict[str, str] = {}

    def flush() -> None:
        if not cur.get("name"):
            return
        stats["entries"] += 1
        entry_board = cur.get("board")
        healthcare = bool(entry_board) and bool(HEALTHCARE_BOARD.search(entry_board))
        if not healthcare or re.search(r"VETERINARY", entry_board or "", re.IGNORECASE):
            stats["skip_non_healthcare"] += 1
            return
        sanction = cur.get("sanction", "")
        if not SERIOUS_SANCTION.search(sanction):
            stats["skip_minor_sanction"] += 1
            return
        records.append(
            {
                "person_or_entity_name": cur["name"].upper(),
                "state": "HI",
                "exclusion_date": _iso_date(cur.get("date", "")),
                "indefinite_exclusion": bool(re.search(r"revoc|revoke", sanction, re.IGNORECASE)),
                "source_id": SOURCE_ID,
                "source_record_id": cur.get("case") or None,
                "provider_type": f"{entry_board} — {sanction}"[:200],
                "source_snapshot_hash": snapshot_hash,
                "observed_at": observed_at,
            }
        )
        stats["written"] += 1

    # Field label -> record key. The value may be inline ("Label: value") OR on
    # the following line(s) — both layouts occur across release pages.
    labels = [("respondent:", "name"), ("case number:", "case"), ("sanction:", "sanction"), ("effective date:", "date")]
    i = 0
    while i < len(lines):
        line = lines[i]
        if _is_heading(line):
            board = line
            i += 1
            continue
        low = line.lower()
        matched = next((key_fld for key_fld in labels if low.startswith(key_fld[0])), None)
        if matched:
            key, fld = matched
            val = line.split(":", 1)[1].strip()
            if not val and i + 1 < len(lines) and not _is_label(lines[i + 1]) and not _is_heading(lines[i + 1]):
                val = lines[i + 1]
                i += 1
            if fld == "name":
                flush()  # previous entry, if any
                cur = {"name": _strip_name(val), "board": board or ""}
            else:
                cur[fld] = val
        i += 1
    flush()
    return records, stats


def _is_label(line: str) -> bool:
    low = line.lower()
    return any(low.startswith(k) for k in ("respondent:", "case number:", "sanction:", "effective date:"))


def _strip_name(raw: str) -> str:
    """Drop alias suffixes (fka/aka/dba) for cleaner name matching."""
    name = re.split(r"\b(?:fka|aka|dba|nka)\b", raw, flags=re.IGNORECASE)[0]
    return name.strip()


def run(data_dir: pathlib.Path, urls: list[str] | None = None) -> dict[str, Any]:
    """Fetch + parse a window of release pages, de-duplicating by (name, date)."""
    urls = urls or DEFAULT_RELEASE_URLS
    raw_dir = data_dir / "raw" / "hawaii_dcca"
    raw_dir.mkdir(parents=True, exist_ok=True)
    observed_at = datetime.now(UTC).isoformat()

    totals = {"pages": 0, "page_errors": 0, "entries": 0, "written": 0, "skip_non_healthcare": 0, "skip_minor_sanction": 0, "duplicates": 0}
    seen: set[tuple[str, str | None]] = set()
    kept: list[dict[str, Any]] = []
    for idx, url in enumerate(urls):
        try:
            raw_html = urlopen(Request(url, headers={"User-Agent": USER_AGENT}), timeout=60).read().decode("utf-8", "replace")
        except Exception:
            totals["page_errors"] += 1
            continue
        totals["pages"] += 1
        snapshot_hash = hashlib.sha256(raw_html.encode("utf-8")).hexdigest()
        (raw_dir / f"disciplinary_actions_{idx}.html").write_text(raw_html, encoding="utf-8")
        records, stats = parse(raw_html, snapshot_hash, observed_at)
        for key in ("entries", "written", "skip_non_healthcare", "skip_minor_sanction"):
            totals[key] += stats[key]
        for r in records:
            dedupe_key = (r["person_or_entity_name"], r["exclusion_date"])
            if dedupe_key in seen:
                totals["duplicates"] += 1
                continue
            seen.add(dedupe_key)
            kept.append(r)

    out_path = data_dir / "normalized" / OUTPUT_NAME
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as out:
        for r in kept:
            out.write(json.dumps(r) + "\n")
    totals["unique_records"] = len(kept)
    return {"out": out_path, "stats": totals}
