# AION-MEDSAFE

[![CI](https://github.com/aion-context/aion-medsafe/actions/workflows/ci.yml/badge.svg)](https://github.com/aion-context/aion-medsafe/actions/workflows/ci.yml)

Agentic evidence and compliance platform for Medicaid integrity teams —
an evidence-ranking and compliance-assistance system with cryptographic
provenance via `aion-context`. It helps investigators find fraud patterns
faster without replacing human judgment; it never accuses autonomously.

- `pipeline/` — Python ingestion, normalization, and entity resolution ([details](pipeline/README.md))
- `system/` — Rust provenance, trust graph, and policy-gated signals
- `.claude/` — Claude Code project configuration

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
