# Contributing to AION-MEDSAFE

Thanks for your interest. This project builds evidence for fraud investigations,
so correctness, provenance, and zero-panic discipline matter more than velocity.
Please read this before opening a PR.

## Ground rules

1. **No real PII/PHI — ever.** Tests and examples use synthetic data only. PRs
   containing real protected data will be closed.
2. **Leads, not accusations.** Keep the no-autonomous-accusation invariant
   (`.claude/rules/agents.md`): nothing escalates without a recorded human step.
3. **Provenance is mandatory.** Don't add a code path that acts on data or policy
   without verification.
4. **Security issues go to `SECURITY.md`, not public issues.**

## Project layout

- `pipeline/` — Python: acquisition + normalization only (no compute).
- `system/` — Rust: all compute + cryptographic provenance.
- `docs/spec/` — the open Sealed Evidence Packet specification.
- `.claude/rules/` — the engineering rules this project enforces (worth reading).

## Local setup

```sh
./scripts/setup.sh        # activate the version-controlled pre-commit hook
cd system && cargo build  # Rust 1.70+
```

## The quality gate (enforced by CI and the pre-commit hook)

Rust (`system/`) follows **Tiger Style** — zero panics:

- No `unwrap`, `expect`, `panic!`, `todo!`, `unreachable!` in production code
  (clippy denies them). Tests may use them.
- `cargo fmt` clean; `cargo clippy -- -D warnings` clean.
- `cargo test` passing; new behavior needs tests (property tests where invariants
  apply — see `.claude/rules/adversarial-testing.md`).
- Functions stay under ~60 lines; structured `tracing` events, not `println!`.

Python (`pipeline/`): `py_compile` clean, `pytest` passing, type hints on public
functions.

Run everything locally:

```sh
make audit     # full integrity + quality sweep
make test      # Rust + Python suites
```

## Pull requests

- Keep PRs focused; explain the *why*.
- Match the surrounding code's style and comment density.
- Update docs/spec when you change the sealed format or a public interface.
- Sign your commits off (`git commit -s`) to certify the
  [Developer Certificate of Origin](https://developercertificate.org/).
- Commit messages: imperative subject; end with the co-author trailer if you used
  an assistant.

## Changing the Sealed Evidence Packet format

The format (`docs/spec/sealed-evidence-packet.md`) is a versioned open standard.
Additive changes (new optional fields) are welcome; breaking changes require a
major-version bump and a migration note. Verifiers must keep ignoring unknown
fields.

## License of contributions

Contributions are accepted under the project's dual **MIT OR Apache-2.0** license
(see `LICENSE-MIT` / `LICENSE-APACHE`). By submitting, you agree your
contribution is licensed under both.
