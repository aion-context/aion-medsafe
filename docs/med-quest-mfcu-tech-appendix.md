# AION-MEDSAFE — Technical & Security Appendix

**For:** State IT Security / CISO / Data Governance reviewing a Med-QUEST pilot
**Companion to:** the positioning brief and one-pager
**Status:** describes the current working prototype. Where something is *designed
but not yet built*, it says so explicitly.
**Example technical appendix — figures illustrative**

---

## 1. Purpose

This appendix is for the technical reviewers who must clear a pilot: it documents
the architecture, the cryptographic provenance model, key management, data
handling and classification, the threat model, supply-chain/build integrity, and
operational requirements. It distinguishes **implemented** from **designed/future**
throughout — particularly around PHI, which is **not in the system today**.

## 2. Architecture

Two components, cleanly separated at a newline-delimited JSON (NDJSON) boundary:

```
 Government bulk files ──► pipeline/ (Python)            system/ (Rust)
   LEIE, NPPES, SAM,        acquire + normalize ONLY     all compute + provenance
   PECOS owners,            → per-source NDJSON      ──►  resolve → graph → signals
   HI exclusions/license                                  → seal (.aion) → packets
```

- **Pipeline (Python):** downloads public bulk files and normalizes them to
  NDJSON. It does **no** entity resolution, scoring, or correlation. This keeps
  the network-facing, format-wrangling code separate from the trust core.
- **System (Rust):** does all computation and all cryptography. Single statically
  compiled binary. No database — state is sealed `.aion` files plus NDJSON.
- **Offline-first compute:** the Rust system makes **no network calls**. Only the
  pipeline reaches the internet, and only to fetch public files; that step can run
  on a separate connected host and the sealed artifacts moved to the compute host.

## 3. Data — sources, classification, handling

**Sources (all public, bulk, no account/API key):** NPPES (CMS), LEIE +
supplements (HHS-OIG), SAM.gov public exclusion extract (data.gov), CMS PECOS
"All Owners" (data.cms.gov), Hawaii Med-QUEST exclusions, Hawaii DCCA/RICO license
discipline. **No PHI, no claims, no beneficiary data.**

**Classification model (enforced by handling + .gitignore):**

| Class | Examples | Handling today |
|---|---|---|
| PUBLIC | LEIE, NPPES, SAM, PECOS owners | stored as raw downloads |
| INTERNAL | normalized NDJSON, signal output | gitignored; not committed |
| SENSITIVE | provider name + exclusion reason combined | never fully logged |
| PROTECTED | claims, beneficiary/patient data | **NOT IN SYSTEM** — future gate |

**What is and isn't committed to source control:**

- **Committed:** source code, the detection *policy source*, and the **public-key
  registry** (public keys only).
- **Never committed (gitignored):** all raw data, all normalized NDJSON, all
  sealed `.aion` artifacts, all private keys, the human-decision logs. A
  pre-commit hook scans staged files for secrets; CI re-checks.

## 4. Cryptographic provenance model

Provenance is provided by the `aion-context` 1.0 library (BLAKE3 hashing,
Ed25519 signatures). Every governance artifact — each ingested source, the
detection policy, the provider graph, each signal run, each human decision, the
release binary attestation — is sealed and, on every use, re-verified against four
guarantees:

1. **Structure valid** — the file parses within declared bounds.
2. **Integrity hash valid** — no byte changed since sealing.
3. **Hash-chain valid** — version lineage is intact (re-ingestion appends a new
   version bound to its parent hash).
4. **Signatures valid** — signed by an authorized author in the registry.

**Policy-gated computation:** the signal engine **refuses to run** if the detection
policy fails verification — it never computes on unsigned or tampered rules
(fail-safe, not fail-open). Trust is never cached; files are re-read and
re-verified on each use.

**Hardening note (disclosed):** we identified a denial-of-service in
`aion-context` 1.0's verifier (a corrupted header could drive an unbounded
allocation). The system mitigates it with a pre-flight header-bounds check before
any file is handed to the verifier. This is our mitigation, not an upstream fix;
it is documented in code and covered by tests.

## 5. Key management & access control

- **Algorithm:** Ed25519 signing keys; 32-byte seeds.
- **AuthorId ranges:** pipeline automation (80001–80009), analysts (80010–80019),
  legal/compliance (80020–80029), system admin (80030–80039).
- **Public vs private:** only **public** keys live in the committed registry.
  Private keys are stored outside the repo (gitignored key files today; OS keyring
  / HSM is the intended production path) and are **never logged or placed in error
  messages**.
- **Externally-held keys:** an analyst can generate a key offline and have only
  the **public** key registered (`keygen` + `register-key`) — the system can admit
  a signer whose private key it never sees (HSM/keyring-friendly), and a rebuilt
  registry can re-admit an existing signer without invalidating past logs.
- **Least privilege:** the pipeline automation key **cannot approve escalations**;
  signing keys are required for every provenance operation (no ambient authority).
- **High-severity actions:** K-of-N multisig is supported for enforcement-grade
  steps.
- **Rotation/revocation:** the registry tracks key epochs and supports revocation.

## 6. Threat model & controls

| Threat | Control |
|---|---|
| Tampering with sealed data/policy/logs | Integrity hash + hash chain → verification fails on any byte change |
| Forged authorship | Ed25519 signature check against the registry |
| Acting on tampered detection rules | Policy gate refuses to compute |
| Malicious/corrupted input file (DoS) | Pre-flight header-bounds guard before verify |
| Unauthorized escalation | Human-in-the-loop + signed decisions; least-privilege keys |
| Autonomous/erroneous accusation | **Design control:** no signal escalates without a named human (ADR-007) |
| Key compromise | Epoch/rotation/revocation in registry; private keys off-repo |
| Supply-chain tampering of the binary | SLSA-style sealed release attestation (see §7) |

## 7. Supply chain & build integrity

- **Dependency pinning:** `Cargo.lock` committed; `aion-context` pinned to an
  explicit version (no wildcards); Python deps pinned.
- **Tiger Style enforcement:** clippy lints **deny** `unwrap`/`expect`/`panic`/
  `todo`/`unreachable` in production code — zero-panic discipline, enforced in CI.
- **CI (GitHub Actions, every push/PR):** `cargo fmt --check`, `cargo clippy -D
  warnings`, `cargo test`, plus Python compile + tests.
- **Mutation + property testing:** property-based tests (proptest/hypothesis) and a
  scheduled `cargo-mutants` job verify the tests actually catch regressions.
- **Release attestation:** the `release` command emits an in-toto/SLSA-style
  provenance statement — binary BLAKE3 digest, package + version, `Cargo.lock`
  digest, source commit — sealed into a verifiable `.aion`. Answers "which exact
  binary, from which dependencies, produced these outputs?"
- **One-command integrity audit:** `make audit` verifies the registry, every
  sealed manifest, code quality, secret scan, and the test suites.

## 8. Deployment & operational requirements

- **Footprint:** one Rust binary + Python 3.13 for the pipeline. No server, no
  database, no external services at compute time.
- **Host:** Linux workstation or VM is sufficient for the pilot (the full national
  build runs in well under a minute and a few hundred MB RAM).
- **Network:** required **only** for the pipeline's public-file downloads; the
  trust/compute path is offline. Air-gapped operation is possible by fetching on a
  connected host and transferring sealed artifacts.
- **Storage:** raw public files (largest is the ~1 GB NPPES bulk) + normalized
  NDJSON + sealed `.aion` outputs. All on local disk; nothing leaves the host.
- **No outbound telemetry.** The system does not phone home.

## 9. Privacy & the PHI/claims future gate

Today the system holds **public provider-level data only** — no claims, no
beneficiary/patient data, no PHI. The dollar-value billing signals require
claims/MMIS data, which is deliberately **out of scope for the pilot**.

When/if claims are introduced (a separate decision), they are a distinct PROTECTED
class requiring — and these are **future requirements, not current features** —
encryption at rest, access control on PHI, audit logging of PHI access, and a
documented minimum-necessary data flow. The architecture reserves this as an
explicit gate rather than assuming it.

## 10. Auditability — how your reviewer verifies it

Given only the **public-key registry**, an independent reviewer can:

- Re-verify any sealed artifact's four guarantees (`provenance` / `verify`
  commands) — offline, months later.
- Confirm a case packet was built from specific sealed sources (hashes are in the
  packet), under a specific signed policy version, reviewed by a specific analyst.
- Re-run `make audit` for a full-system integrity sweep.

Provenance is therefore **third-party checkable**, not self-asserted.

## 11. Known limitations / items for IT review

- **`aion-context` dependency:** MIT/Apache-2.0; the DoS mitigation above is ours,
  pending an upstream fix — worth your dependency review.
- **Name-only correlations:** CMS ownership and state license data lack NPIs, so
  those matches are name-based; the system suppresses collision-prone matches and
  labels the rest "verify manually." They are leads, not conclusions.
- **PHI architecture is designed, not built** (see §9).
- **Key storage** today uses gitignored files; production should use the OS keyring
  or an HSM (supported via the public-key registration path).

## 12. Dependency inventory (high level)

- **Rust:** `aion-context` (provenance), `blake3`, Ed25519 (via aion-context),
  `serde`/`serde_json`/`serde_yaml`, `clap`, `chrono`, `csv`, `hex`, `anyhow`,
  `thiserror`, `tracing`. Pinned in `Cargo.lock`.
- **Python:** standard library for ingestion; `typer`/`rich` for the CLI;
  `pytest`/`hypothesis` for tests. Pinned.

---

*All claims reflect the current working system over current public data. Source
repository, `Cargo.lock`, CI configuration, and the audit script are available for
your review.*
