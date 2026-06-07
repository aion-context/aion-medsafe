---
description: Compute risk signals from the Trust Graph using verified detection policy. Use when the user asks to run signals, compute risk, or detect anomalies.
argument-hint: "[jurisdiction]"
---

# Risk Signal Computation

Computes policy-gated risk signals AND an entity-resolution review queue over a
sealed Trust Graph. Every input is a verified `.aion`; the output is sealed.

## Prerequisites
- Registry initialized (`system/.aion/medsafe.registry.json`)
- Detection policy sealed as `.aion` (`policy/detection_policy.aion`)
- Sealed Trust Graph (`provenance/trust_graph.aion`)
- NPPES national status table built via `aion-medsafe-pipeline nppes-bulk`
  (powers `active_npi_while_excluded`; bulk-first, full coverage)

## Steps

### 1. Build + seal the Trust Graph (if stale)
Resolves entities (NPI/name hard merges + fuzzy linking) over the normalized
NDJSON and seals the graph. Skip if `provenance/trust_graph.aion` is current.
```bash
cd system && ./target/release/aion-medsafe build-graph \
  --normalized ../pipeline/data/normalized \
  --output provenance/trust_graph.aion
```

### 2. Seal the detection policy (if not already sealed)
```bash
cd system && ./target/release/aion-medsafe seal-policy \
  --rules policy/detection_policy.yaml \
  --output policy/detection_policy.aion
```

### 3. Compute signals + review queue
`signals` re-verifies BOTH the policy and the graph `.aion` (all four
guarantees) before computing — it REFUSES on any failure.
```bash
cd system && ./target/release/aion-medsafe signals \
  --policy policy/detection_policy.aion \
  --graph provenance/trust_graph.aion \
  --jurisdiction $ARGUMENTS
```

## Output (sealed to `provenance/signals_{jurisdiction}_{ts}.aion`)

NDJSON audit records, each distinguished by `reason_code`:

| reason_code | Meaning |
|---|---|
| `signal_queued_review` | Risk signal at/above threshold → human review |
| `signal_below_threshold` | Risk signal logged, not escalated |
| `identity_review_candidate` | Two entities that may be the same provider — confirm/reject (NOT auto-merged) |

The run-meta line carries counts: `signal_count`, `queued_for_review`,
`identity_review_candidates`, and `not_computable`.

## Signal Types (from policy)
- `re_exclusion` — Reinstated then excluded again
- `multi_state_exclusion` — Exclusions in multiple states
- `federal_state_mismatch` — On federal LEIE but absent from the state list
- `active_npi_while_excluded` — Excluded but NPI active in NPPES
- `npi_deactivation_after_exclusion` — NPI deactivated *after* exclusion (possible attempt to disappear)
- `shared_practice_location` — Excluded providers sharing a practice address/phone (shell clinic / ownership network)
- `colocated_active_providers` — Excluded provider's address/phone shared by ACTIVE non-excluded NPIs (practice may still be operating under other identities)
- `billing_after_exclusion` — Needs claims data (reported not-computable until ingested)

## Tracking change over time
Diff two sealed signal runs to surface only the delta (newly flagged / resolved /
changed) — the actionable output for a recurring re-scan or monthly data refresh:
```bash
cd system && ./target/release/aion-medsafe diff \
  --from provenance/signals_hi_<earlier>.aion \
  --to   provenance/signals_hi_<later>.aion
```

## Case packets (the investigator deliverable)
Turn flagged providers into court-defensible dossiers — identity + signals +
exclusion evidence (with source provenance hashes) + the policy version + a
verification footer — sealed to `.aion` and rendered to Markdown.
```bash
cd system && ./target/release/aion-medsafe packet \
  --policy policy/detection_policy.aion \
  --graph provenance/trust_graph.aion \
  --jurisdiction HI [--entity <id>] [--limit N]
```

## The identity review queue
Entity resolution surfaces sub-auto-merge links (e.g. `WOLF ROBERT A ⇄ WOLF
ROBERT`) for a human to confirm or reject. Confirming a link can change which
signals fire, so triage these alongside risk signals. The queue is
jurisdiction-filtered (a candidate appears if either entity has a nexus to the
jurisdiction) and ordered by confidence.

### Acting on the queue (closes the loop)
First, enroll each reviewer once (generates + registers their signing key):
```bash
cd system && ./target/release/aion-medsafe enroll-analyst --author 80010
```
For an externally-held key (HSM/keyring/another machine), generate it offline and
register only the PUBLIC key — the system never sees the private key:
```bash
cd system
./target/release/aion-medsafe keygen --out /secure/analyst.key   # prints pubkey hex
./target/release/aion-medsafe register-key --author 80011 --pubkey <hex>
```
Record a verdict; it is signed with that analyst's key and appended to a sealed,
hash-chained decision log. Re-run `build-graph` to apply: confirmed links force a
merge, rejected links are kept separate and suppressed from the queue.
```bash
cd system
# confirm two entities are the same provider (signed by analyst 80010)
./target/release/aion-medsafe decide --a "<entity_id_a>" --b "<entity_id_b>" \
  --decision confirm --author 80010 --reason "<why>"
# or reject
./target/release/aion-medsafe decide --a "<id_a>" --b "<id_b>" --decision reject --author 80010
./target/release/aion-medsafe decisions          # list current verdicts
./target/release/aion-medsafe build-graph         # apply (confirm→merge, reject→suppress)
```
The decision log (`decisions/identity_decisions.aion`) is authoritative
operational state — not regenerable. Each decision is cryptographically
attributable: signed by the reviewing analyst's enrolled key (author 80010+),
verified against the registry on load.
