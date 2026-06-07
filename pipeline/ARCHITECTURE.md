# Pipeline Architecture

## Purpose

The AION-MEDSAFE pipeline is a Python ETL subsystem dedicated to continuously refreshing public datasets and consolidating them into normalized, provenance-preserving records for the Rust application layer.

## Separation of Concerns

### Python Pipeline

The Python pipeline is responsible for:

- Fetching raw public datasets
- Tracking freshness and source availability
- Hashing raw snapshots
- Preserving source provenance
- Parsing heterogeneous formats such as API JSON, CSV, ZIP, PDF, and downloadable public files
- Normalizing records into stable interchange contracts
- Exporting immutable datasets for Rust services

### Rust System

The Rust system is responsible for:

- Provider Trust Graph runtime behavior
- Case and investigation workflow
- User-facing APIs
- Authorization and human approval gates
- Evidence custody enforcement
- Audit log persistence and serving
- Operational reliability of the core application

## Data Flow

```text
Official public source
        ↓
Python fetcher
        ↓
Raw immutable snapshot + SHA-256
        ↓
Source-specific parser
        ↓
Normalized provider / exclusion / license / enforcement records
        ↓
Versioned export boundary
        ↓
Rust AION-MEDSAFE services
```

## Freshness Contract

Every source should define:

- Source owner
- Official URL
- Access method
- Expected refresh cadence
- Last successful fetch timestamp
- Last content hash
- Staleness threshold
- Failure behavior

If a source becomes stale, the pipeline should emit an explicit freshness failure rather than silently continuing with old data.

## Provenance Contract

Every normalized record should retain:

- Source dataset identifier
- Source record identifier where available
- Fetch timestamp
- Source URL
- Raw snapshot hash
- Parser version
- Normalization version

Derived risk signals must be traceable back to the source records that produced them.

## Rust Handoff Contract

Rust should consume normalized, versioned exports. It should not depend on raw public-source formats directly.

Recommended handoff formats:

- NDJSON for simple streaming ingestion
- Parquet for analytical and batch workloads
- SQLite for MVP local validation
- Object-store snapshot manifests for evidence custody workflows

## Failure Modes

- Source unavailable
- Source schema changes
- PDF layout changes
- Partial download
- Hash mismatch
- Duplicate records
- Conflicting identities
- Stale source data
- Ambiguous entity resolution

All failures should be explicit, observable, and reproducible.
