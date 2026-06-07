# AION-MEDSAFE — Claude Code Project Context

## What This Is

AION-MEDSAFE is an **agentic evidence and compliance platform for Medicaid integrity teams**. It helps investigators find fraud patterns faster without replacing human judgment.

This is NOT a "fraud accusation AI." It is an **evidence-ranking and compliance-assistance system** with cryptographic provenance via `aion-context`.

## Architecture

```
aion-medsafe/
├── pipeline/         Python — bulk data ingestion, normalization, entity resolution
├── system/           Rust — cryptographic provenance, trust graph, policy-gated signals
├── .claude/          Claude Code configuration (this folder)
└── prd0.md           Product requirements document
```

### Pipeline (Python) — acquisition + normalization only
- Ingests government bulk files (LEIE, NPPES, SAM.gov, state exclusion lists)
- Normalizes to per-source NDJSON (the handoff to the Rust system)
- Deliberately thin: NO entity resolution, graph building, or correlation —
  all compute lives in the Rust system (one fast stack over verified data)

### System (Rust) — provenance + all compute
- Depends on `aion-context` 1.0 for tamper-evident provenance
- Seals every ingested file with BLAKE3 hash + Ed25519 signature
- Entity resolution (`resolve.rs`): NPI/exact-name hard merges + multi-signal
  fuzzy linking with phonetic blocking; sub-merge matches surfaced for review
- Trust Graph build (`build.rs`): reads normalized NDJSON → resolves →
  reconstructs lifecycle events → seals the graph into a `.aion`
- Policy-gated signal generation (refuses to act if policy is tampered)
- Tiger Style: zero panics, all paths return `Result<T, E>`

## Critical Invariants

1. **No autonomous accusation.** Every risk signal requires human review before escalation.
2. **Provenance is mandatory.** Every data source must be sealed with aion-context before use.
3. **Policy gates computation.** Detection rules live in signed `.aion` files. System REFUSES to compute signals if policy verification fails.
4. **Evidence chain of custody.** Every decision, source, model output, override, and approval is logged.
5. **No silent failure.** Every error is surfaced, logged, and traceable.

## Key Data Sources

| Source | Records | Format | Scope |
|--------|---------|--------|-------|
| LEIE (HHS-OIG) | 83,256 | CSV | National exclusions |
| NPPES Deactivated | 343,322 | Excel | Deactivated NPIs |
| SAM.gov Exclusions | ~150,000 | CSV | Federal procurement exclusions |
| Hawaii Med-QUEST | ~200 | PDF | State Medicaid exclusions |
| LEIE Supplements | Monthly | CSV | Recent exclusions/reinstatements |

## Technology Stack

- **Rust 1.70+** — system core, aion-context integration
- **Python 3.13** — pipeline, data engineering
- **aion-context 1.0** — tamper-evident policy files, Ed25519 + BLAKE3
- **BLAKE3** — content-addressed hashing
- **Ed25519** — signing (with ML-DSA-65 hybrid future path)

## AuthorId Allocation

| Range | Role |
|-------|------|
| 80001–80009 | Pipeline automation |
| 80010–80019 | Analyst operators |
| 80020–80029 | Legal/compliance |
| 80030–80039 | System admin |

## Skills (Slash Commands)

- `/ingest [file] [source_id]` — Bulk data ingestion with provenance sealing
- `/verify [manifest]` — Provenance verification (4 guarantees)
- `/signal [jurisdiction]` — Policy-gated risk signal computation
- `/audit` — Full system integrity audit
- `/test` — Run full test suite (unit + property-based)
- `/deploy` — Build release binary and package
- `/new-source [id] [url]` — Onboard a new data source (full checklist)

## Rules (auto-loaded from `.claude/rules/`)

Rules are automatically loaded by Claude Code when working in this project:
- `tiger-style.md` — Zero panics, bounded resources, observability
- `security.md` — Key handling, data classification, signing verification
- `provenance.md` — Chain of custody protocol
- `adversarial-testing.md` — Property-based and mutation testing gates
- `agents.md` — No autonomous accusation, decision protocol
- `data-engineering.md` — Bulk-first, schema discipline
- `aion-context-patterns.md` — Library usage patterns and anti-patterns
- `expert-team.md` — 6 specialist personas
- `domain-knowledge.md` — Medicaid fraud terminology and law
