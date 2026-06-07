"""PDF Parser Validation with confidence scoring and drift detection.

De-risks the Hawaii Med-QUEST PDF fragility problem.

Techniques:
- Schema expectation checks (expected columns, expected page count range)
- Record count confidence scoring (compare to prior known counts)
- Structural fingerprinting (detect layout changes before they corrupt data)
- Parse confidence per-record (flag low-confidence extractions)
- Drift alerts (warn when structure deviates from baseline)
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ParseConfidence:
    """Confidence score for a single parsed record."""
    record_index: int
    raw_text: str
    confidence: float  # 0.0 to 1.0
    issues: list[str] = field(default_factory=list)

    @property
    def is_reliable(self) -> bool:
        return self.confidence >= 0.7


@dataclass
class PDFValidationReport:
    """Validation report for an entire PDF parse run."""
    source_url: str
    page_count: int
    raw_record_count: int
    reliable_record_count: int
    low_confidence_records: list[ParseConfidence]
    structural_drift_detected: bool
    drift_details: list[str]
    overall_confidence: float

    @property
    def is_trustworthy(self) -> bool:
        return self.overall_confidence >= 0.8 and not self.structural_drift_detected


@dataclass
class PDFSchemaExpectation:
    """Baseline expectations for a PDF source. Built from prior successful parses."""
    source_id: str
    expected_page_count_range: tuple[int, int] = (3, 10)
    expected_record_count_range: tuple[int, int] = (150, 300)
    expected_header_tokens: list[str] = field(default_factory=lambda: [
        "Last Name or Business Name",
        "First Name",
        "Exclusion Date",
        "Reinstatement",
    ])
    expected_date_pattern: str = r"\d{1,2}/\d{1,2}/\d{2,4}"
    min_records_with_dates: float = 0.9  # at least 90% should have dates


# ─── Structural Fingerprint ───────────────────────────────────────────────────


def compute_structural_fingerprint(pages_text: list[str]) -> dict[str, Any]:
    """Compute a structural fingerprint of the PDF for drift detection."""
    fingerprint: dict[str, Any] = {
        "page_count": len(pages_text),
        "avg_lines_per_page": 0,
        "header_present_on_pages": [],
        "date_density": 0.0,
    }

    total_lines = 0
    date_pat = re.compile(r"\d{1,2}/\d{1,2}/\d{2,4}")
    date_count = 0

    for i, text in enumerate(pages_text):
        lines = text.split("\n")
        total_lines += len(lines)
        # Check header presence
        if "Last Name or Business Name" in text:
            fingerprint["header_present_on_pages"].append(i)
        # Count dates
        date_count += len(date_pat.findall(text))

    fingerprint["avg_lines_per_page"] = total_lines / max(len(pages_text), 1)
    fingerprint["date_density"] = date_count / max(total_lines, 1)

    return fingerprint


# ─── Validation ───────────────────────────────────────────────────────────────


def validate_pdf_parse(
    pages_text: list[str],
    parsed_records: list[dict[str, Any]],
    expectation: PDFSchemaExpectation,
    source_url: str,
) -> PDFValidationReport:
    """Validate a PDF parse against expectations and produce a confidence report."""
    drift_details: list[str] = []
    structural_drift = False

    # Page count check
    page_count = len(pages_text)
    pmin, pmax = expectation.expected_page_count_range
    if not (pmin <= page_count <= pmax):
        drift_details.append(
            f"Page count {page_count} outside expected range [{pmin}, {pmax}]"
        )
        structural_drift = True

    # Record count check
    record_count = len(parsed_records)
    rmin, rmax = expectation.expected_record_count_range
    if not (rmin <= record_count <= rmax):
        drift_details.append(
            f"Record count {record_count} outside expected range [{rmin}, {rmax}]"
        )
        structural_drift = True

    # Header token check
    full_text = "\n".join(pages_text)
    for token in expectation.expected_header_tokens:
        if token not in full_text:
            drift_details.append(f"Expected header token missing: '{token}'")
            structural_drift = True

    # Per-record confidence scoring
    date_pat = re.compile(expectation.expected_date_pattern)
    record_confidences: list[ParseConfidence] = []

    for i, record in enumerate(parsed_records):
        issues: list[str] = []
        score = 1.0

        name = record.get("person_or_entity_name", "")
        excl_date = record.get("exclusion_date", "")

        # Name should not be empty
        if not name or len(name) < 2:
            issues.append("name_too_short")
            score -= 0.4

        # Name should not contain date-like strings (parser error)
        if date_pat.search(name):
            issues.append("name_contains_date")
            score -= 0.3

        # Exclusion date should be present and valid
        if not excl_date:
            issues.append("missing_exclusion_date")
            score -= 0.3
        elif not date_pat.match(excl_date):
            issues.append("invalid_date_format")
            score -= 0.2

        # Name too long usually means multi-record merge error
        if len(name) > 80:
            issues.append("name_suspiciously_long")
            score -= 0.3

        score = max(score, 0.0)
        record_confidences.append(ParseConfidence(
            record_index=i,
            raw_text=name[:100],
            confidence=score,
            issues=issues,
        ))

    reliable_count = sum(1 for rc in record_confidences if rc.is_reliable)
    low_confidence = [rc for rc in record_confidences if not rc.is_reliable]

    # Overall confidence
    if record_count == 0:
        overall = 0.0
    else:
        overall = reliable_count / record_count
        if structural_drift:
            overall *= 0.7  # penalize for structural drift

    return PDFValidationReport(
        source_url=source_url,
        page_count=page_count,
        raw_record_count=record_count,
        reliable_record_count=reliable_count,
        low_confidence_records=low_confidence,
        structural_drift_detected=structural_drift,
        drift_details=drift_details,
        overall_confidence=overall,
    )
