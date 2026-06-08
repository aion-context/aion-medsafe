# Security Policy

AION-MEDSAFE produces evidence used in fraud investigations. Security is a core
requirement, not an afterthought. Thank you for helping keep it sound.

## Reporting a vulnerability

**Please do NOT open a public issue for security vulnerabilities.**

Report privately to **dj@codetestcode.io** with:

- a description of the issue and its impact,
- steps to reproduce (a minimal proof-of-concept if possible),
- affected component (`pipeline/`, `system/`, a sealed-format/spec issue, etc.),
- any suggested remediation.

We aim to acknowledge within **5 business days** and to agree on a disclosure
timeline with you. We support **coordinated disclosure** and will credit
reporters who wish to be named.

**Never include real PII or PHI in a report.** Use synthetic data to demonstrate
issues (see `.claude/rules/testing.md`). Reports containing real protected data
will be deleted unread and asked to be resubmitted with synthetic fixtures.

## Scope

In scope:
- Provenance / verification correctness (the four guarantees: structure,
  integrity, hash chain, signatures).
- Cryptographic key handling, the policy gate, and the Sealed Evidence Packet
  format (`docs/spec/sealed-evidence-packet.md`).
- Parsing of untrusted input (data files, sealed `.aion` files).
- Resource-exhaustion / denial-of-service in any parser or verifier.

Out of scope:
- The security of third-party government data sources themselves.
- Issues requiring a compromised host or stolen private signing key (these are
  outside the trust boundary; key custody is the operator's responsibility).

## Known issues / mitigations

- **`aion-context` 1.0 verifier DoS.** A corrupted header could drive an
  unbounded allocation. AION-MEDSAFE mitigates this with a pre-flight
  header-bounds check before any file reaches the verifier
  (`system/src/provenance.rs`). This is our mitigation pending an upstream fix;
  reports of bypasses are especially welcome.

## Supported versions

This is pre-1.0 software. Only the latest `main` is supported. Pin a commit for
reproducibility and watch the repository for security updates.

## Handling of data

- The system processes **public provider data only**; it holds no claims or
  beneficiary/PHI today (see the technical appendix, §9).
- Private signing keys, raw data, normalized output, and sealed artifacts are
  gitignored and MUST NOT be committed. A pre-commit hook and CI both scan for
  secrets.
