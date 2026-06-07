# Architecture Decision Records

## ADR-001: Bulk Files Over APIs
**Decision:** Prefer raw government bulk files over API calls.
**Rationale:** No rate limiting, no external dependency during processing, full control over data, repeatable ingestion.
**Consequence:** Must track download URLs and file format changes manually.

## ADR-002: National Scope, Local Focus
**Decision:** Ingest national data, filter to jurisdiction at query time.
**Rationale:** Fraud doesn't respect state boundaries. A provider excluded in one state may be billing in another. National ingestion catches cross-state evasion.
**Consequence:** Larger data volume, but enables detection of patterns invisible to single-state systems.

## ADR-003: aion-context for Provenance
**Decision:** Use `aion-context` 1.0 for all cryptographic provenance.
**Rationale:** Provides tamper-evident, hash-chained, signed policy files with zero-copy parsing, offline-first operation, and auditor-ready verification.
**Consequence:** Rust dependency, but the library is MIT/Apache-2.0 with zero panics and stable 1.0 API.

## ADR-004: Python Pipeline + Rust System Split
**Decision:** Data engineering in Python, trust/provenance in Rust.
**Rationale:** Python excels at data wrangling (pandas, CSV, PDF parsing). Rust excels at cryptographic correctness with zero panics and type safety.
**Consequence:** Two-language stack, but cleanly separated at the NDJSON boundary.

## ADR-005: NDJSON as Interchange Format
**Decision:** All pipeline outputs are NDJSON (newline-delimited JSON).
**Rationale:** Streamable, line-oriented (easy to count, grep, split), self-describing schema, works with every tool.
**Consequence:** Slightly larger than binary formats, but human-readable and debuggable.

## ADR-006: Policy-Gated Computation
**Decision:** Risk signal computation REFUSES to proceed if detection policy fails verification.
**Rationale:** A system that computes risk signals on tampered policy is worse than no system at all — it could generate false accusations or miss real fraud.
**Consequence:** System downtime if policy is corrupted, but this is the correct behavior (fail-safe, not fail-open).

## ADR-007: No Autonomous Accusation
**Decision:** Every risk signal above threshold requires human review before escalation.
**Rationale:** Legal, ethical, and practical. Fraud accusations have severe consequences. AI-generated accusations without human oversight are indefensible in court and ethically unacceptable.
**Consequence:** Human bottleneck in the escalation path, but this is a feature, not a bug.

## ADR-008: Content-Addressed Canonical IDs
**Decision:** Provider entities get deterministic canonical IDs derived from content hash.
**Rationale:** Same data always produces same ID. No coordination needed. Enables deduplication and cross-reference without a central ID authority.
**Consequence:** IDs are opaque hashes, not human-friendly. Need a lookup layer for display.
