# Provenance Rules — Chain of Custody

## Scope
All data ingestion and signal computation paths.

## Core Principle
Every piece of data the system acts on must have cryptographic proof of origin.

## Ingestion Protocol

1. Download raw file from government source
2. Compute BLAKE3 hash of raw bytes BEFORE any transformation
3. Create `ArtifactManifest` with `(name, size, hash)` triple
4. Sign manifest with pipeline key
5. Create `.aion` provenance file via `init_file`
6. Store provenance in `provenance/` directory
7. ONLY THEN proceed with normalization/transformation

## Verification Protocol

Before using any data file for computation:
1. Load registry from `.aion/medsafe.registry.json`
2. Call `verify_file(manifest_path, &registry)`
3. Check ALL FOUR guarantees:
   - `structure_valid` — file parses correctly
   - `integrity_hash_valid` — no byte has been modified
   - `hash_chain_valid` — version history is intact
   - `signatures_valid` — signed by authorized author
4. If ANY guarantee fails → abort with structured error

## Version Commits

When data is re-ingested (monthly refresh):
- Use `commit_version` (not `init_file`) to append to existing provenance
- This builds the hash chain: v1 → v2 → v3 → ...
- Each version's `parent_hash` binds to the previous version
- The chain proves the complete ingestion history

## What the Provenance Proves

| Question | Answer From |
|---|---|
| "What file did you ingest?" | `rules_hash` in version entry |
| "When did you ingest it?" | `timestamp` in version entry |
| "Who authorized the ingestion?" | `author_id` + signature |
| "Has anyone tampered with it since?" | `integrity_hash_valid` |
| "Is this the same file as last month?" | Compare `rules_hash` across versions |

## File Naming Convention

```
provenance/{source_id}_{YYYY-MM}.aion
```

Examples:
- `provenance/leie_updated_2026-06.aion`
- `provenance/nppes_deactivated_2026-05.aion`
- `provenance/hawaii_medquest_2026-04.aion`
