# AION-MEDSAFE

[![CI](https://github.com/aion-context/aion-medsafe/actions/workflows/ci.yml/badge.svg)](https://github.com/aion-context/aion-medsafe/actions/workflows/ci.yml)

Agentic evidence and compliance platform for Medicaid integrity teams —
an evidence-ranking and compliance-assistance system with cryptographic
provenance via `aion-context`. It helps investigators find fraud patterns
faster without replacing human judgment; it never accuses autonomously.

> **Disclaimer.** AION-MEDSAFE is an independent open-source prototype. It is
> **not affiliated with, endorsed by, or operated on behalf of** any government
> agency. It uses **only public data**, holds no claims or protected health
> information, and its outputs are **investigative leads for human review — not
> findings or accusations**. Agency names appear only to describe public data
> sources and context.

- `pipeline/` — Python ingestion, normalization, and entity resolution ([details](pipeline/README.md))
- `system/` — Rust provenance, trust graph, and policy-gated signals
- `docs/` — [documentation index](docs/README.md), incl. the open
  [Sealed Evidence Packet spec](docs/spec/sealed-evidence-packet.md)
- `.claude/` — Claude Code project configuration

## Documentation

- **Spec:** [Sealed Evidence Packet (SEP/0.1)](docs/spec/sealed-evidence-packet.md)
  — the open, verifiable evidence format.
- **Pilot materials:** [one-pager](docs/med-quest-mfcu-onepager.md) ·
  [brief](docs/med-quest-mfcu-brief.md) ·
  [technical appendix](docs/med-quest-mfcu-tech-appendix.md).
- Full index: [`docs/README.md`](docs/README.md).

## Development setup

After cloning, run the bootstrap script once to activate the version-controlled
git hooks (this sets `core.hooksPath`, which is per-clone local config and is
not committed):

```sh
./scripts/setup.sh
```

This enables the pre-commit gate (`scripts/hooks/pre-commit`): `cargo clippy
-- -D warnings` + `cargo fmt --check` on staged Rust, `py_compile` on staged
Python, and a secret scan. Commits are blocked if any check fails.

## Contributing & security

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — setup, the quality gate, the
  no-real-PII rule, and DCO sign-off.
- [`SECURITY.md`](SECURITY.md) — private vulnerability reporting (do **not** open
  public issues for security).
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — Contributor Covenant.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option. The Sealed Evidence Packet specification (`docs/spec/`) is
published under Apache-2.0 to encourage independent implementations. Unless you
state otherwise, any contribution you submit is dual-licensed as above.
