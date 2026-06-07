# AION-MEDSAFE Pipeline

Python ETL freshness pipeline for ingesting public Medicaid integrity, provider identity, exclusion, licensing, and enforcement datasets.

## Boundary

The pipeline owns:

- Raw public data acquisition
- Source freshness checks
- Provenance metadata
- Normalization into stable interchange models
- Dataset snapshots for downstream Rust services

The Rust system owns:

- Case workflow
- Provider Trust Graph runtime
- Investigation APIs
- Human review and approvals
- Evidence custody enforcement
- Audit log serving

## Initial Source Priorities

1. NPPES / NPI Registry
2. HHS-OIG LEIE exclusions
3. Hawaii Med-QUEST exclusion and reinstatement list
4. SAM.gov exclusions
5. CMS public provider enrollment / PECOS-derived files

## Output Contract

Pipeline outputs should be versioned, immutable, and source-attributed. Rust should consume normalized records from a stable export boundary rather than scraping raw source formats directly.

Candidate export formats:

- NDJSON for append-friendly records
- Parquet for analytical workloads
- SQLite for local MVP validation
- Object-store snapshots for sealed evidence workflows

## Run

```bash
PYTHONPATH=src python3 -m aion_medsafe_pipeline.cli sources
```
