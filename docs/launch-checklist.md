# Launch Roadmap

A sequenced plan for taking AION-MEDSAFE public and growing it responsibly.
`[x]` = done; `[ ]` = to do. Items are tagged **(maintainer)** for GitHub-UI /
outreach steps, or **(repo)** for in-tree work.

---

## Phase 0 — Foundations ✓

- [x] Dual license (`LICENSE-APACHE` + `LICENSE-MIT`)
- [x] `SECURITY.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`
- [x] `.github/` PR + issue templates (security routed privately)
- [x] CI on `main` (fmt · clippy `-D warnings` · tests; Python suite)
- [x] `make audit` integrity sweep passing
- [x] Open spec published in-repo: **SEP/0.1** (`docs/spec/`)
- [x] Documentation index (`docs/README.md`)
- [x] Git history reviewed — no keys, data, PHI, or secrets in any commit
- [x] Strategy & pilot docs reviewed and prepared for public release

## Phase 1 — Final pre-public touches

- [ ] **(repo)** Affiliation/disclaimer in the README: independent prototype, **not
      affiliated with or endorsed by** any government agency; **public data only**;
      outputs are investigative leads, not findings.
- [ ] **(repo)** Mark any illustrative pilot figures (success criteria, timelines)
      clearly as examples.
- [ ] **(repo)** Set version `0.1.0` in `system/Cargo.toml`; add `CHANGELOG.md`.
- [ ] **(maintainer)** Re-run `make audit`; confirm CI green on the publish commit.

## Phase 2 — Go public

- [ ] **(maintainer)** GitHub → Settings → change visibility to **Public**.
- [ ] **(maintainer)** Set repo description + topics (`medicaid`, `fraud`,
      `program-integrity`, `provenance`, `rust`, `chain-of-custody`).
- [ ] **(maintainer)** Confirm the CI badge renders publicly.

## Phase 3 — Post-public hardening

- [ ] **(maintainer)** Enable **secret scanning** + push protection.
- [ ] **(maintainer)** Enable **Dependabot** alerts + security updates.
- [ ] **(maintainer)** **Branch protection** on `main`: require CI + PRs.
- [ ] **(maintainer)** Enable GitHub **private vulnerability reporting**
      (complements `SECURITY.md`).

## Phase 4 — Release `v0.1.0`

- [ ] **(maintainer)** Create the GitHub Release from the tag, with notes
      (capabilities, the eight signals, the SEP spec, "prototype" framing).
- [ ] **(repo/maintainer)** Build the release binary and run `make release` to
      produce the SLSA-style attestation; attach the binary + attestation `.aion`
      + the public-key registry as **release assets** so the build is verifiable.
- [ ] **(repo)** Verify the README quick start works from a clean clone on
      synthetic fixtures.

## Phase 5 — Standard / spec (the strategic core)

- [ ] **(repo)** Promote **SEP/0.1** as a citable artifact (stable link + README
      section).
- [ ] **(maintainer)** Announce in relevant integrity / health-IT / OSS channels;
      **invite independent implementations + feedback**.
- [ ] **(maintainer)** Open an RFC issue on the spec (`spec`/`rfc` labels).
- [ ] **(repo, optional)** Ship a standalone **`sep-verify`** example so anyone can
      verify a packet without the full system.

## Phase 6 — Adoption & outreach

- [ ] **(maintainer)** Offer no-cost evaluation to integrity units rebuilding
      tooling (leads-only, public data, no procurement) — respectfully, as a help.
- [ ] **(maintainer)** Monitor opportunities: SAM.gov, Med-QUEST solicitations
      (RFIs precede RFPs), HIePRO / HANDS.
- [ ] **(maintainer)** Prepare RFI-response material that encourages
      verifiable-chain-of-custody as a requirement.
- [ ] **(maintainer)** Explore partnerships where the provenance/evidence layer can
      strengthen an existing analytics platform.

## Phase 7 — Ongoing

- [ ] **(maintainer)** Triage issues / security reports per `SECURITY.md`.
- [ ] **(repo)** Keep CI green; address Dependabot; keep `Cargo.lock` current.
- [ ] **(repo)** Monthly data-refresh cadence; re-run `make audit` after refreshes.
- [ ] **(maintainer)** Document one real, defensible case file as a reference proof.

---

> **Highest-leverage path:** make verifiable evidence an open standard (Phase 5)
> and earn a real reference deployment (Phase 6). Everything else supports those.
