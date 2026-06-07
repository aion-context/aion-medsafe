---
paths:
  - "pipeline/src/**/*.py"
  - "pipeline/tests/**/*.py"
  - "pipeline/pyproject.toml"
---

# Data Engineering Rules

## Scope
Python pipeline code in `pipeline/src/`.

## Bulk-First Principle

- ALWAYS prefer raw government flat files over APIs
- No rate limiting, no external dependencies, full control
- Sources: CSV, Excel, ZIP archives, PDFs (when no alternative)
- Store originals in `data/raw/` — never modify raw files

## Schema Discipline

- All normalized data follows the schema in `schema.py`
- Three core entities: `ProviderEntity`, `ExclusionEvent`, `RiskSignal`
- Every record has a `canonical_id` (deterministic, content-addressed)
- Entity resolution maps across sources using NPI, name, DOB, state

## NDJSON as Interchange Format

- All normalized outputs are NDJSON (one JSON object per line)
- Store in `data/normalized/`
- File naming: `{source_type}_normalized.ndjson`
- Each line is self-contained (no cross-line references)

## Transformation Rules

- Preserve all source fields (even if currently unused)
- Add metadata: `source`, `ingested_at`, `raw_file_hash`
- Normalize dates to ISO 8601
- Normalize names to uppercase (matching government convention)
- Normalize state codes to 2-letter abbreviations

## Cross-Reference Protocol

When matching across sources:
1. Primary key: NPI (10-digit, unique per provider)
2. Secondary: name + state + DOB
3. Fuzzy: name similarity + address proximity (confidence-scored)
4. Every match carries a `confidence` field (0.0–1.0)
5. Matches below `minimum_confidence_for_alert` threshold are logged but not acted on

## Error Handling

```python
# NEVER silently skip bad records
try:
    record = parse_row(row)
except ParseError as e:
    logger.warning("parse_failed", row_num=i, error=str(e), source=source_id)
    stats["parse_errors"] += 1
    continue  # skip but COUNT it
```

- Always report: total records, processed, skipped, errors
- If error rate > 5% of total → HALT and alert (data source may have changed format)
