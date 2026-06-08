# Public-Launch Checklist

**Internal working checklist.** Sequenced from "ready to flip" to "launched and
maintained." `[x]` = done; `[ ]` = to do. Tasks are tagged **(you)** for
GitHub-UI / outreach steps only you can do, or **(repo)** for code/doc work that
can be done in-tree.

---

## Phase 0 — Already done ✓

- [x] Dual license (`LICENSE-APACHE` + `LICENSE-MIT`), consistent with SPDX headers
- [x] `SECURITY.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`
- [x] `.github/` PR + issue templates (security routed privately)
- [x] CI green on `main` (fmt · clippy `-D warnings` · tests; Python suite)
- [x] `make audit` PASS (22 sealed manifests verified)
- [x] Open spec published in-repo: **SEP/0.1** (`docs/spec/`)
- [x] Documentation index (`docs/README.md`)
- [x] **Git history scanned — clean** (no keys, data, PHI, or secrets in any commit)

## Phase 1 — Pre-flip blockers (DO BEFORE going public)

- [ ] **(repo) Decide on internal docs.** `docs/go-to-market.md` is marked
      "internal — not for distribution"; the pilot brief/one-pager/appendix are
      "Draft — internal." Going public exposes them (incl. competitive strategy).
      Options:
      - **Remove** them from the repo before flipping (and purge from history if
        they must never be public — the repo is private now, so history is still
        contained), **or**
      - **Keep** them public deliberately (transparency can help a standards play)
        after a wording pass to drop the "internal" framing and anything you don't
        want a competitor reading.
- [ ] **(repo) Add an affiliation/disclaimer** to the README: independent
      prototype, **not affiliated with or endorsed by** any government agency;
      uses **public data only**; outputs are investigative leads, not findings.
      (The repo names real agencies and a real MFCU action — this protects against
      misrepresentation.)
- [ ] **(repo) Drop placeholder figures** in any docs that stay public (success
      criteria, timelines) or mark them clearly illustrative.
- [ ] **(repo) Tag a release version** — set `0.1.0` in `system/Cargo.toml`,
      add a short `CHANGELOG.md`.
- [ ] **(you) Re-run** `make audit` and confirm CI green on the exact commit you
      will publish.

## Phase 2 — The flip (you)

- [ ] **(you)** GitHub → repo **Settings → General → Danger Zone → Change
      visibility → Public**. (One-way-ish; do it from the reviewed commit.)
- [ ] **(you)** Set repo **description** + **topics** (`medicaid`, `fraud`,
      `program-integrity`, `provenance`, `rust`, `chain-of-custody`).
- [ ] **(you)** Confirm the CI badge renders publicly.

## Phase 3 — Immediate post-flip hardening (you)

- [ ] **(you)** Enable **Secret scanning** + **push protection** (free on public).
- [ ] **(you)** Enable **Dependabot** alerts + security updates.
- [ ] **(you)** **Branch protection** on `main`: require the CI check to pass;
      require PRs; (optionally) require signed commits.
- [ ] **(you)** Add a **SECURITY** advisory contact / enable private vulnerability
      reporting in repo settings (complements `SECURITY.md`).

## Phase 4 — Release (mix)

- [ ] **(you)** Create GitHub **Release `v0.1.0`** from the tag, with notes
      (capabilities, the 8 signals, the SEP spec, "prototype" framing).
- [ ] **(repo/you)** Build the release binary and run `make release` to produce
      the **SLSA-style attestation**; attach the binary + attestation `.aion` +
      the public-key registry as **release assets** so the build is verifiable.
- [ ] **(repo)** Ensure README "quick start" works from a clean clone
      (`./scripts/setup.sh`, `cargo build`, a sample run on synthetic fixtures).

## Phase 5 — Standard / spec launch (the strategic core)

- [ ] **(repo)** Promote **SEP/0.1** as a citable artifact (stable link; consider
      a short landing section in the README).
- [ ] **(you)** Announce where the audience is (relevant integrity/health-IT/OSS
      channels); explicitly **invite independent implementations + feedback**.
- [ ] **(you)** Open a "request for comment" issue on the spec; label
      `spec`/`rfc`.
- [ ] **(repo, optional)** Ship a tiny standalone **`sep-verify`** example so
      others can verify a packet without the full system — lowers adoption cost
      and reinforces "the verifier is open."

## Phase 6 — Go-to-market / the MFCU wedge (you)

- [ ] **(you)** Identify a champion in the **Hawaii AG / rebuilding MFCU**
      ("developing tools for investigation") — offer a free, no-procurement pilot.
- [ ] **(you)** Monitor opportunities: **SAM.gov** Contract Opportunities,
      **Med-QUEST** solicitations (RFIs precede RFPs), **HIePRO/HANDS**.
- [ ] **(you)** Prepare RFI-response boilerplate that pushes
      verifiable-chain-of-custody into requirements.
- [ ] **(you)** Line up **teaming** conversations with a prime (you bring
      provenance to their PI/MMIS bid).

## Phase 7 — Ongoing (mix)

- [ ] **(you)** Triage issues / security reports per `SECURITY.md` SLA.
- [ ] **(repo)** Keep CI green; address Dependabot; keep `Cargo.lock` current.
- [ ] **(repo)** Monthly data-refresh cadence; re-run `make audit` after refreshes.
- [ ] **(you)** Track one real, cited case file as the reference proof point.

---

> **Highest-leverage path (from the GTM doc):** make verifiable evidence the open
> standard (Phase 5) **and** land the defunded MFCU as a zero-friction reference
> (Phase 6). Everything else supports those two.
