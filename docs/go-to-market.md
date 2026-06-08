# AION-MEDSAFE — Go-to-Market & Open-Source Strategy

**Internal strategy — NOT for distribution.** Candid working document.
**Thesis in one line:** A solo builder can't out-contract the incumbents — but by
making *verifiable evidence* an open standard and landing a zero-friction
reference deployment, you can change what the market is required to compete on.

---

## 1. The market reality

Medicaid program-integrity / fraud-analytics is dominated by a few primes whose
moat is **relationships, MMIS contracts, certifications, and scale** — not
technology you can't match:

| Vendor | Position | The gap they don't lead with |
|---|---|---|
| **Gainwell** (ex-HMS) | "The incumbent"; analytics partner to ~52 state Medicaid programs | Cryptographic chain of custody / hearing-grade evidence |
| **Optum** | State Program Integrity suite, scale | Same |
| **Conduent** | **Hawaii's current MMIS fiscal agent** (local incumbent) | Same |
| **SAS** | AI/ML fraud scoring | Same |
| **LexisNexis / IBM / Deloitte / Pondera / Cotiviti** | Data, SI muscle, niche scoring | Same |

They all sell **more scoring**. None of them lead with provenance. A solo will not
win an MMIS or PI prime contract head-to-head — and shouldn't try.

## 2. The asymmetric thesis

Three things a solo structurally has that the incumbents don't:

1. **Redefine the requirement.** You can't sell to 50 states, but you can shift the
   *spec*. If "third-party-verifiable, hearing-grade evidence" becomes what buyers
   ask for, every prime must answer to your framing.
2. **Be the cited expert, not the line item.** A public, working artifact + the
   standard around it → you're who CMS/OIG/AGs call. Converts to consulting, a
   role, an acquihire, or a reference deployment faster than a sales pipeline.
3. **Give it away where they can't.** Their model is expensive multi-year
   contracts. A cash-strapped unit can deploy a free, auditable tool with a
   champion. Free distribution is a weapon exactly where incumbents are weakest.

## 3. The wedge (timely — act on it)

Hawaii's **MFCU was decertified and defunded (announced June 4, 2026)** after
**zero convictions / zero indictments 2021–2025**; the AG says they are
"developing tools for investigation." That public failure is a *defensibility/
evidence* gap — not a scoring gap — which is precisely what AION-MEDSAFE produces.

A defunded unit **cannot buy a Gainwell contract** but **can adopt a free,
auditable tool**. This is the lowest-friction entry point in the market right now.

## 4. Open-source strategy: what to open vs hold

A chain-of-custody tool is **more** credible open ("don't trust us, read the
verifier"). Closed provenance is a contradiction. But open-source ≠ revenue, so
draw the open-core boundary deliberately:

| Open (spread the standard) | Commercial (capture the value) |
|---|---|
| The `.aion` provenance + **sealed case-packet/evidence spec** | Data pipelines & source connectors |
| The **verifier** (re-check the four guarantees) | Analyst workflow + UI |
| The core detection engine + CLI | Hosting / SLA / support / on-call |
| Test suites, audit script | Managed pilots & integration services |
| Docs / format reference | Certification + **you as the expert** (highest margin for a solo) |

**Goal of the open half:** become the *SARIF-of-Medicaid-evidence* — the format
auditors expect and procurements cite. That standard authorship is the durable
category win, even if a prime implements it.

## 5. License matrix

| Component | License | Why |
|---|---|---|
| Provenance format + evidence spec + verifier | **Apache-2.0** | Maximize adoption; let everyone (incl. primes) produce/verify the format → it becomes the standard |
| Engine / server (if run as a service) | **AGPL-3.0** | A prime running it as a closed SaaS must contribute back — prevents pure extraction |
| Name / logo | **Trademark** | Protect the brand even under permissive code |

(Org is `copyleftdev` — the Apache-on-the-standard / AGPL-on-the-server split fits
both the "spread the standard" and "don't get strip-mined" goals.)

## 6. Value capture (how a solo actually gets paid)

Revenue does **not** come from the code; it comes from what organizations pay for
around it:
- **Expert services / consulting** — highest margin, lowest overhead, immediate.
- **Managed pilots** — run the pilot for the agency; convert to support.
- **Hosting + support/SLA** — the open-core SaaS line.
- **Connectors / integration** — state-specific data plumbing.
- **Certification / training** — "AION-verified evidence" program.
- **Teaming fees** — bring provenance to a prime's PI bid as a sub.

## 7. Go-to-market sequence

1. **Release** the OSS core + the open evidence-packet **spec**, framed around
   *defensible cases that convict* (the exact public failure in Hawaii).
2. **Land the wedge** — free pilot to the **AG / rebuilding MFCU** ("developing
   tools for investigation"); no procurement, no incumbent fight.
3. **Produce one cited case file** as proof → your reference + your authority.
4. **Expand two ways:** (a) **team with primes** — you bring provenance to their
   PI/MMIS bids; (b) **open-core company** — hosting/services to other states.
5. **Influence the spec** — answer Med-QUEST 2026 **RFIs**, and push CMS/OIG
   guidance toward verifiable chain-of-custody so procurements *require* it.

## 8. Where opportunities post (monitor)

- **Federal:** SAM.gov → Contract Opportunities; grants.gov.
- **Hawaii Med-QUEST:** medquest.hawaii.gov solicitations (RFIs precede RFPs — the
  moment to shape requirements).
- **Hawaii statewide:** HIePRO (sub-threshold), HANDS (awards).
- The pilot path (defunded MFCU, free tool) likely bypasses formal procurement via
  small-purchase / sole-source thresholds — confirm with their procurement office.

## 9. Risks & mitigations (honest)

| Risk | Mitigation |
|---|---|
| OSS produces no revenue by itself | Value capture lives in services/hosting/standard-stewardship, not secrecy |
| A prime absorbs the format | That's also standard validation; capture value via certification + being the author/expert; AGPL on the server |
| "Free" reads as "not serious" to gov | Pair with paid managed pilot + SLA; lead with the audit/provenance rigor |
| Gov adoption still needs a champion | The MFCU wedge *is* the champion search; don't expect passive uptake |
| Security scrutiny / disclosure burden | Already Tiger-Style + tested + CI; publish a security policy + disclosure process |
| Solo bandwidth | Stay narrow: own the *standard + the wedge*, partner for scale |

## 10. Realistic outcome ladder (for a solo)

1. **Authority** → consulting / a role / acquihire. *(most likely, soonest)*
2. **Reference deployment** (MFCU rebuild) → credibility + case study.
3. **Standard adoption** → category influence; procurements cite your format.
4. **Open-core company** → recurring services/hosting revenue. *(largest, slowest)*

OSS accelerates 1–3, which are the achievable, high-value rungs for one person.

## 11. Immediate next actions

- [ ] Decide license split (Apache core/spec + AGPL server) and add `LICENSE` +
      `SECURITY.md` + `CONTRIBUTING.md`.
- [ ] Extract the **evidence-packet spec** as a standalone, citable document.
- [ ] Re-angle the brief toward the **AG/MFCU rebuild** + add the competitive
      landscape section (this doc feeds it).
- [ ] Identify one champion in the AG's office / rebuilt MFCU; offer a free pilot.
- [ ] Prepare RFI-response boilerplate for Med-QUEST 2026 solicitations.

---

*Sources for the market/procurement reads are in the chat research log; key facts:
Gainwell ≈52 state programs and "the incumbent"; Conduent = Hawaii MMIS fiscal
agent; Hawaii MFCU decertified/defunded June 2026 with zero 2021–2025 convictions.
All AION-MEDSAFE capability claims reflect the current working system. — Draft, internal*
