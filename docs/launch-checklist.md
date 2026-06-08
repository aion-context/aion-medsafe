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

## Phase 1 — Final pre-public touches ✓

- [x] **(repo)** Affiliation/disclaimer in the README + site footer: independent
      prototype, **not affiliated with or endorsed by** any government agency;
      **public data only**; outputs are investigative leads, not findings.
- [x] **(repo)** Illustrative pilot figures marked as examples.
- [x] **(repo)** Version `0.1.0` (both crates) + `CHANGELOG.md`.
- [x] **(maintainer)** `make audit` PASS; CI green.

## Phase 2 — Go public ✓

- [x] **(maintainer)** Repo visibility set to **Public**.
- [x] **(maintainer)** Description + homepage + 20 discovery topics set; SEO meta,
      JSON-LD, robots.txt, sitemap, and a PNG OG card shipped.
- [x] **(maintainer)** Pages live at https://aion-context.github.io/aion-medsafe/
      (Actions deploy), CI badge renders publicly.

## Phase 3 — Post-public hardening ✓

- [x] **(maintainer)** Secret scanning + push protection enabled.
- [x] **(maintainer)** Dependabot alerts + security updates enabled.
- [x] **(maintainer)** Branch protection on `main`: CI required (strict),
      linear history, force-push + deletion blocked; admin bypass kept for the
      solo maintainer (external changes still go through PR + CI).
- [x] **(maintainer)** Private vulnerability reporting enabled (complements
      `SECURITY.md`).

## Phase 4 — Release `v0.1.0` ✓

- [x] **(maintainer)** Published GitHub Release `v0.1.0` (notes from
      `docs/release-notes-v0.1.0.md`).
- [x] **(repo/maintainer)** `make release` SLSA attestation + binary + public-key
      registry attached as verifiable assets.
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
