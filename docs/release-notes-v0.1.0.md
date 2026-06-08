# AION-MEDSAFE v0.1.0

First public release — a working **prototype** of a provenance-first evidence and
lead-ranking system for Medicaid provider integrity. It assembles public federal
and state adverse-action data, ranks cross-corroborated leads for **human review**,
and produces **court-defensible, independently verifiable** case files. It never
accuses autonomously.

> **Disclaimer.** Independent open-source prototype. **Not affiliated with or
> endorsed by** any government agency. Uses **public data only**; holds no claims
> or PHI. Outputs are **investigative leads for human review — not findings**.

## Highlights

- **7 bulk data sources** (no account): NPPES, LEIE + supplements, SAM.gov
  exclusions, CMS PECOS ownership, Hawaii Med-QUEST & DCCA license discipline.
- **8 detection signals** + an excluded-owner correlation — evidence-ranked,
  policy-gated, human-reviewed (no autonomous accusation).
- **Court-defensible case packets** — identity, signals (with *earned* precision),
  evidence with source hashes, federal/state/license coverage, ownership, and a
  verification footer.
- **Sealed chain of custody** — BLAKE3 + Ed25519 (via `aion-context`); four
  verification guarantees; policy-gated computation.
- **Calibration loop**, **snapshot diff**, **SLSA-style release attestation**.
- **Open standard:** [Sealed Evidence Packet (SEP/0.1)](spec/sealed-evidence-packet.md).

## Verify this release

This project is about verifiable provenance, so the release is verifiable too:

```sh
# 1. Confirm the binary matches the sealed attestation's subject digest.
b3sum aion-medsafe        # compare to subject.digest.blake3 in the .aion attestation
# 2. Verify the attestation itself against the public-key registry.
aion-medsafe provenance --manifest aion-medsafe_0.1.0.aion
```

Release assets: the Linux binary, the SLSA-style attestation
(`aion-medsafe_0.1.0.aion`), and the public-key registry
(`medsafe.registry.json`). *(Binary is Linux/x86-64; build from source for other
platforms — `cargo build --release`, Rust 1.70+.)*

## Quick start (from source)

```sh
git clone https://github.com/aion-context/aion-medsafe && cd aion-medsafe
./scripts/setup.sh
cd system && cargo build --release
./target/release/aion-medsafe --help
make audit        # full integrity + quality sweep
```

## Known limitations

- **No claims/MMIS analysis** — PHI is an explicit future gate; current outputs
  are provider-integrity leads, not billing findings.
- **Ownership & state-license correlations are name-based** (those sources lack
  NPIs); low-precision matches are labeled or suppressed, not asserted.

## Docs

[Documentation index](README.md) · [SEP spec](spec/sealed-evidence-packet.md) ·
[Security policy](../SECURITY.md) · [Contributing](../CONTRIBUTING.md) ·
[Changelog](../CHANGELOG.md)
