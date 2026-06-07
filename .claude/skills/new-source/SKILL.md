---
description: Onboard a new government bulk data source into the pipeline with full provenance. Use when the user wants to add a new data source, register a new feed, or integrate a new dataset.
argument-hint: "[source_id] [download_url]"
---

# Add a New Data Source

## Checklist (all steps mandatory)

### 1. Register in Bulk Source Registry
Add entry to `pipeline/src/aion_medsafe_pipeline/bulk_sources.py`

### 2. Write Parser
Create or update `pipeline/src/aion_medsafe_pipeline/parsers.py`:
- Handle the source's specific format
- Normalize to `ExclusionEvent` or `ProviderEntity` schema
- Preserve all source fields in `raw_fields` dict

### 3. Write Tests
Minimum 3 tests:
- Parse a known-good sample → correct field extraction
- Parse malformed input → graceful error with count
- Round-trip: raw → normalized → verify field preservation

### 4. Download Raw File
```bash
cd pipeline/data/raw && curl -L -O "$1"
```

### 5. Seal Provenance
```bash
cd system && ./target/release/aion-medsafe ingest \
  --file ../pipeline/data/raw/<filename> \
  --source $0
```

### 6. Verify
```bash
cd system && ./target/release/aion-medsafe provenance \
  --manifest provenance/$0.aion
```

### 7. Update Documentation
- Add source to `CLAUDE.md` data sources table
- Add source to `ARCHITECTURE.md`

## Acceptance Criteria
- [ ] Source registered in `bulk_sources.py`
- [ ] Parser handles format correctly
- [ ] NDJSON output matches schema
- [ ] Tests pass (3+ cases)
- [ ] Provenance sealed (4/4 guarantees pass)
- [ ] Documentation updated
