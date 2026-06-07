---
description: Compute risk signals from the Trust Graph using verified detection policy. Use when the user asks to run signals, compute risk, or detect anomalies.
argument-hint: "[jurisdiction]"
---

# Risk Signal Computation

## Prerequisites
- Registry initialized (`system/.aion/medsafe.registry.json`)
- Detection policy sealed as `.aion` file
- Data sources ingested with provenance

## Steps

### 1. Verify Detection Policy
```bash
cd system && ./target/release/aion-medsafe provenance \
  --manifest policy/detection_rules.aion
```
Must show `Valid: true`. If not — STOP. Do not compute signals on tampered policy.

### 2. Verify Data Source Provenance
```bash
cd system
for f in provenance/*.aion; do
  ./target/release/aion-medsafe provenance --manifest "$f" 2>&1 | grep "Valid:"
done
```
All must pass. Any failure — exclude that source from computation.

### 3. Compute Signals
```bash
cd system && ./target/release/aion-medsafe signals \
  --policy policy/detection_rules.aion \
  --graph pipeline/data/normalized/trust_graph.ndjson \
  --jurisdiction $ARGUMENTS
```

### 4. Threshold Classification
| Confidence | Action |
|---|---|
| < 0.75 | Log only, no human review |
| 0.75–0.90 | Queue for analyst review |
| > 0.90 | Priority queue + immediate notification |
| 1.0 (deterministic) | Auto-escalate to legal |

## Signal Types (from policy)
- `federal_state_mismatch` — On LEIE but not state list
- `active_npi_while_excluded` — Excluded but NPI active
- `billing_after_exclusion` — Claims after exclusion date
- `re_exclusion` — Reinstated then excluded again
