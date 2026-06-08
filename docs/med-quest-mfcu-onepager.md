# AION-MEDSAFE
### Defensible provider-integrity leads for Hawaii Med-QUEST & the MFCU

**Find the strongest provider-integrity leads faster — and hand investigators a
court-defensible evidence file.** AION-MEDSAFE assembles the scattered federal and
state adverse-action data, ranks cross-corroborated leads for human review, and
seals every step so the chain of custody holds up in a hearing. It never accuses
on its own.

---

**The problem.** The signals already exist — an excluded provider still billing, a
clinic reborn under new NPIs, a barred owner behind an active facility — but they
live in mismatched datasets with no shared IDs. Assembling them by hand is slow,
and a spreadsheet won't survive a defense challenge.

**What it does (today, on real public data):**

- Unifies **9.55M** NPI records, **83K** federal exclusions, **6.7K** SAM.gov
  debarments, **790K** Medicare ownership links, plus Hawaii's exclusion and
  license-discipline lists — resolved to one provider graph.
- Runs **8 evidence-ranked signals** (e.g. *active NPI while excluded*, *clinic
  still operating under other NPIs*, *flagged by multiple independent oversight
  bodies*, *excluded owner of an active provider*).
- Produces a **sealed, court-ready case packet** per flagged provider: identity,
  signals, every piece of evidence **with its source hash**, and a verification
  footer an auditor can re-check months later, offline.

**Why it's different — verifiable chain of custody.** Every source, rule, and
human decision is hash-chained and signed (BLAKE3 + Ed25519). Opposing counsel can
independently confirm a case file was built from exactly the government data we
claim, under signed rules, reviewed by a named analyst. Not a black-box score.

**Built for due process.** No lead escalates without a named human; that review is
itself signed and logged. Designed against **42 CFR Part 455**. Public data only —
claims/PHI are a separate, deliberate future gate.

**Honest about limits.** Today's output is **provider-integrity leads, not billing
findings** — claims analysis needs MMIS data we haven't ingested. Name-only
correlations (ownership, licensing) are labeled "verify manually," not asserted.

---

### The ask: a ~30-day, leads-only pilot (no PHI)

| You provide | You receive |
|---|---|
| A few analysts to adjudicate a lead batch | A ranked Hawaii lead queue |
| Your Med-QUEST exclusion list (any format) | Sealed case packets for the top leads |
| A legal reviewer for 1–2 packets | A calibration report from *your* verdicts + a chain-of-custody assessment |

**Success, set with you:** defensible leads not already in your queue · reduced
analyst time-to-packet · legal sign-off that a packet is hearing-grade.

*Working prototype, running on current public data. Full brief and technical
detail on request. — Draft, internal*
