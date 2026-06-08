# AION-MEDSAFE — Positioning Brief

**For:** Hawaii Med-QUEST Program Integrity & the Medicaid Fraud Control Unit (MFCU)
**Re:** A provenance-first evidence and lead-ranking system for provider integrity
**Status:** Working prototype on real national data — pilot-ready
**Example proposal — figures illustrative**

---

## Executive summary

AION-MEDSAFE helps integrity staff **find the strongest provider-integrity leads
faster, and hand investigators a defensible evidence file** — without ever making
an accusation on its own. It ingests the public federal and state adverse-action
data (exclusions, deactivations, federal procurement debarments, Medicare
ownership, state license discipline), resolves providers across those sources,
and ranks cross-corroborated leads for human review. Every source, decision, and
output is **cryptographically sealed** (hash-chained + signed), so the chain of
custody is verifiable by a third party — built for an administrative hearing or a
courtroom, not just a dashboard.

It is **not** a "fraud AI." No lead escalates without a named human reviewer, and
that review is itself signed and logged.

---

## The problem

Provider-integrity teams already know the patterns — an excluded provider still
billing, a clinic reconstituted under new NPIs, a barred owner behind an active
facility. The bottleneck is **assembly and defensibility**:

- The signals live in **separate, mismatched federal and state datasets** (LEIE,
  NPPES, SAM.gov, PECOS ownership, state Medicaid and licensing lists) that don't
  share identifiers.
- Manually cross-referencing them is slow, and the result is a spreadsheet — not
  something whose provenance survives a defense challenge.
- Tooling that "scores" providers with opaque models is **indefensible in a due-
  process setting** and ethically wrong for accusations with this much weight.

## What AION-MEDSAFE is (and is not)

| It IS | It is NOT |
|---|---|
| An evidence-ranking + lead-prioritization assistant | A decision-maker or accuser |
| A verifiable chain of custody over public data | A black-box risk score |
| Human-in-the-loop by design (ADR-007) | Autonomous enforcement |
| A way to produce court-ready case files | A replacement for investigators |

## What it does today (on real data)

Running now over current public bulk data — no accounts, no APIs, fully repeatable:

- **9.55M** national NPI records (NPPES), **83K** federal exclusions (LEIE) +
  monthly supplements, **6.7K** healthcare-relevant SAM.gov federal exclusions,
  **790K** CMS PECOS ownership relationships, plus Hawaii Med-QUEST exclusions and
  Hawaii DCCA license-discipline actions.
- **Entity resolution** across all of it (NPI + name/phonetic matching), with
  sub-threshold links surfaced for a human to confirm — never auto-merged.
- **Eight detection signals**, each evidence-ranked and policy-gated, e.g.:
  - *Active NPI while excluded* — excluded provider whose billing NPI is still active.
  - *Billing-after-exclusion network* — clinics still operating under other NPIs at
    the same address/phone.
  - *Cross-authority corroboration* — flagged by ≥2 independent oversight tiers
    (federal, state Medicaid, state licensing).
  - *Excluded owner* — an excluded party owning an active Medicare provider.
- **Court-defensible case packets** — one dossier per flagged provider: identity,
  the signals (with their *earned* historical precision), every piece of evidence
  **with its source hash**, federal/state/license coverage, ownership, and a
  verification footer. Sealed and reproducible.
- **A calibration loop** — when a reviewer marks a lead a true/false positive
  (signed), the system reports per-signal-type precision, so confidence is earned
  from your outcomes, not asserted.

## The differentiator: defensible chain of custody

Every artifact — each ingested source, the detection policy, the provider graph,
the signal output, each human decision — is sealed with **BLAKE3 hashing + Ed25519
signatures** (via the `aion-context` provenance library). This yields four
guarantees on every file, re-checked on every use:

1. **Integrity** — no byte changed since sealing.
2. **Authenticity** — signed by an authorized author (analyst keys are individual).
3. **History** — a hash chain proves the full version lineage.
4. **Policy gating** — the engine **refuses to compute** if the detection policy
   fails verification (no acting on tampered rules).

Practically: an auditor or opposing counsel can take the public key registry and
**independently verify that a case file was built from exactly the government data
we claim, under exactly the signed rules, reviewed by exactly the named analyst** —
months later, offline.

## Hawaii fit

- Scoped to **Med-QUEST** (~400K beneficiaries, 5 managed-care plans). National
  data is ingested but filtered to Hawaii nexus at query time, so **cross-state
  evasion is visible** (a provider excluded elsewhere but active here).
- Already ingests the two Hawaii sources that have no machine-readable feed —
  Med-QUEST exclusions and DCCA/RICO license discipline — and folds them into the
  same graph as the federal data.
- Aligns with the program's known risk areas (behavioral health, home health,
  personal-care services), where shell-clinic and ownership patterns are common.

## Honest limitations (what it does NOT do yet)

We would rather state these up front than oversell:

- **No claims analysis yet.** The dollar-value fraud signals (upcoding, phantom
  billing, billing-after-exclusion *confirmation*) require claims/MMIS data, which
  is PHI and behind a deliberate access gate. Today's signals are **provider-
  integrity leads**, not billing findings.
- **Some correlations are name-only.** CMS publishes ownership and the state
  publishes license discipline **without NPIs**, so matching those to the exclusion
  universe is name-based. We **suppress collision-prone matches and clearly label
  the rest "verify manually"** — they are leads, not conclusions.
- **It produces leads, not verdicts.** Precision is reported honestly and improves
  as reviewers adjudicate.

## Legal & compliance posture

- **No autonomous accusation** (ADR-007) — every above-threshold signal queues for
  a named human; approvals/rejections are signed and sealed.
- **Due process aware** — outputs are investigative leads; the system records, not
  decides. Designed against **42 CFR Part 455**.
- **Chain of custody** — evidence is sealed and timestamped before reference.
- **Least privilege** — pipeline automation keys cannot approve escalations;
  high-severity actions support K-of-N multisig.
- **Data classification** — public data only today; claims/PHI are a separate,
  explicit future gate, not assumed.

## Proposed pilot

**Goal:** prove, on Hawaii data, that the system surfaces real, defensible leads
faster than the current manual process — and that the case packets hold up to
your legal review.

- **Scope:** provider-integrity leads only (no claims/PHI). Public + state
  exclusion/licensing/ownership data already in hand.
- **What we'd need from you:** a few analysts to adjudicate a batch of leads
  (to calibrate precision), your current Med-QUEST exclusion list in any format,
  and a legal reviewer to pressure-test one or two case packets.
- **Timeline (illustrative):** ~30 days. Week 1 stand-up + ingest; Weeks 2–3
  analyst review + calibration; Week 4 legal review of packets + readout.
- **You receive:** a ranked Hawaii lead queue, sealed case packets for the top
  leads, a calibration report (per-signal precision from *your* verdicts), and a
  written limitations + chain-of-custody assessment.
- **Success criteria (set with you):** e.g. ≥N defensible leads not already in your
  queue; analyst time-to-packet reduced; legal sign-off that a packet is
  hearing-grade.

## Who should see this

- **Med-QUEST Program Integrity** — lead prioritization, cross-state visibility.
- **MFCU** — court-defensible evidence files, chain of custody.
- **Compliance / legal** — the provenance and due-process posture.

## Why now

The federal and state adverse-action data is more open than it has ever been
(CMS made Medicare ownership public; SAM and exclusion lists are downloadable in
bulk). The missing piece has been a way to **assemble it defensibly**. That is
exactly what AION-MEDSAFE is built to do — and it runs on this data today.

---

*Prototype repository and full technical detail available on request. All figures
above reflect the current working system over current public data.*
