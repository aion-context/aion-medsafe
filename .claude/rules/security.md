# Security Rules — Non-Negotiable

## Scope
All code in this repository.

## Cryptographic Material

- **NEVER** hardcode signing keys, API keys, or secrets in source code
- Keys live in `.aion/pipeline.key` (gitignored) or OS keyring
- Registry files (`.aion/medsafe.registry.json`) contain only public keys — safe to commit
- When referencing keys in logs, use truncated hex (first 8 bytes max)

## Data Classification

| Classification | Examples | Handling |
|---|---|---|
| **PUBLIC** | LEIE CSV, NPPES data, SAM.gov | May be stored in `data/raw/` |
| **INTERNAL** | Normalized NDJSON, risk signals | Store in `data/normalized/`, gitignore |
| **SENSITIVE** | Provider names + exclusion reasons combined | Never log full records |
| **PROTECTED** | Patient data, claims records | NOT YET IN SYSTEM — future gate |

## Access Control Principles

- No ambient authority: signing keys required for every provenance operation
- Principle of least privilege: pipeline automation key (80001) cannot approve escalations
- Key rotation: epochs tracked in registry, revocation supported
- Multisig for high-severity: 2-of-3 required for enforcement actions

## Supply Chain

- Pin all Rust dependencies with `Cargo.lock` (committed)
- Pin all Python dependencies with exact versions
- `aion-context` version must be explicit (no `*` or `>=`)
- New dependencies require justification (what it does, why needed, alternatives considered)

## Signing Verification

Before acting on any data:
1. Verify `.aion` manifest integrity (all 4 guarantees)
2. If verification fails → REFUSE to proceed
3. Log the failure with structured fields
4. Never cache trust ("I verified this last time" is not acceptable)

## Forbidden Patterns

```rust
// NEVER: trust without verification
let data = load_file("policy.yaml"); // unsigned, anyone could edit

// ALWAYS: verify then trust
let report = verify_file(Path::new("policy.aion"), &registry)?;
if !report.is_valid { return Err(...); }
let data = show_current_rules(Path::new("policy.aion"))?;
```
