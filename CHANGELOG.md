# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-07

First public release. A working prototype over real public data, with a full
sealed chain of custody from raw source to court-defensible case packet.

### Data sources (bulk-first, public, no account)
- NPPES national NPI status table (CMS bulk dissemination).
- LEIE federal exclusions + monthly supplements (HHS-OIG).
- SAM.gov exclusions public extract (data.gov mirror); healthcare-relevant subset.
- CMS PECOS "All Owners" ownership data for six provider types (data.cms.gov).
- Hawaii Med-QUEST exclusions and Hawaii DCCA/RICO license discipline.

### Detection (eight signals, evidence-ranked, policy-gated, human-reviewed)
- `re_exclusion`, `multi_state_exclusion`, `federal_state_mismatch`,
  `active_npi_while_excluded`, `npi_deactivation_after_exclusion`,
  `shared_practice_location`, `colocated_active_providers`,
  `adverse_action_coverage`.
- `billing_after_exclusion` reported as not-computable (requires claims data).
- Excluded-owner correlation against CMS ownership (name-precision tiered + honest
  suppression of collision-prone matches).

### Provenance & trust
- Tamper-evident sealing (BLAKE3 + Ed25519 via `aion-context`) with four
  verification guarantees; policy-gated computation (refuses on tampered policy).
- Pre-flight header-bounds guard mitigating an `aion-context` 1.0 verifier DoS.
- Per-analyst signing keys; `keygen` + `register-key` for externally-held keys;
  least-privilege author ranges.

### Workflow & outputs
- Entity resolution with a human-reviewed identity-link queue (`decide`).
- Court-defensible **case packets** (`packet`) — identity, signals, evidence with
  source hashes, federal/state/license coverage, folded-in ownership, attestation.
- **Calibration loop** (`adjudicate` / `calibrate`) — earned per-signal precision.
- **Snapshot diff** (`diff`) of two signal runs.
- **SLSA-style release attestation** (`release`).

### Specification
- **Sealed Evidence Packet (SEP/0.1)** open specification (`docs/spec/`).

### Tooling & quality
- Tiger Style (zero-panic; clippy `-D warnings`); 67 Rust unit + 6 integration +
  19 Python tests; property-based tests.
- GitHub Actions CI (fmt · clippy · tests; Python) + scheduled mutation testing.
- One-command integrity audit (`make audit`).

### Documentation
- Med-QUEST/MFCU positioning brief, one-pager, and technical/security appendix;
  strategy & open-source approach; launch roadmap.

### Known limitations
- No claims/MMIS analysis (PHI is an explicit future gate); current outputs are
  provider-integrity leads, not billing findings.
- Ownership and state-license correlations are name-based (no NPI in those
  sources); low-precision matches are labeled or suppressed, not asserted.

[0.1.0]: https://github.com/aion-context/aion-medsafe/releases/tag/v0.1.0
