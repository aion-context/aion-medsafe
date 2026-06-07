---
description: Ingest a government bulk data file with full provenance sealing. Use when the user wants to import new data, ingest a CSV/Excel file, or add a new data source.
argument-hint: "[file_path] [source_id]"
---

# Bulk Data Ingestion Workflow

## Steps

1. Identify the source file and its type from the bulk source registry (`pipeline/src/aion_medsafe_pipeline/bulk_sources.py`)

2. Verify the file exists and check its size:
```bash
ls -lh $0
wc -l $0  # if CSV
```

3. Run the Rust ingestion with provenance sealing:
```bash
cd system && ./target/release/aion-medsafe ingest \
  --file $0 \
  --source $1
```

4. Verify the provenance was sealed correctly:
```bash
cd system && ./target/release/aion-medsafe provenance \
  --manifest provenance/$1.aion
```

5. Confirm all 4 guarantees pass:
   - Structure: true
   - Integrity: true
   - Hash chain: true
   - Signatures: true

6. Run the Python normalization pipeline:
```bash
cd pipeline && python -m aion_medsafe_pipeline.cli normalize \
  --source $1 \
  --input $0 \
  --output data/normalized/$1_normalized.ndjson
```

7. Report final counts:
   - Raw records
   - Normalized records
   - Parse errors (if any)
   - BLAKE3 hash of raw file
   - Provenance manifest path

## Source IDs
- `leie_updated` — LEIE master exclusion list
- `nppes_deactivated` — NPPES deactivated NPIs
- `sam_exclusions` — SAM.gov federal exclusions
- `hawaii_medquest` — Hawaii state exclusion list
- `leie_supplements` — Monthly LEIE updates
