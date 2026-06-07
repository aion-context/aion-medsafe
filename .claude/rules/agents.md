# Agent Architecture Rules

## Scope
All agentic behavior in AION-MEDSAFE.

## Core Constraint: No Autonomous Accusation

An AI agent in this system MUST NEVER:
- Accuse a provider of fraud without human review
- Suspend, decertify, or take enforcement action autonomously
- Generate external communications (letters, reports) without human approval
- Override a human reviewer's decision

## Agent Roles

| Agent | Function | Autonomy Level |
|---|---|---|
| Intake Agent | Parse complaints, referrals, tips | Full auto: parse + classify |
| Claims Anomaly Agent | Detect billing patterns | Auto-detect, human-approve |
| Provider Risk Agent | Score provider risk | Auto-score, human-threshold |
| Evidence Custody Agent | Seal evidence, maintain timeline | Full auto: cryptographic ops |
| Policy Compliance Agent | Check rules, flag violations | Auto-flag, human-verify |
| Investigation Copilot | Summarize cases, draft packets | Assist only: human drafts |

## Decision Protocol

```
Signal Generated (automated)
  → Confidence Score Computed (automated)
  → Threshold Check (automated — policy-gated)
  → Below threshold? → Log + archive (no human needed)
  → Above threshold? → Queue for Human Review
    → Human approves? → Escalate + seal decision
    → Human rejects? → Log rejection + reason + seal
```

## Policy-Gated Execution

Every agent MUST:
1. Load its policy from a verified `.aion` file
2. Refuse to act if policy verification fails
3. Log every decision with: `action`, `reason`, `policy_version`, `confidence`
4. Operate within bounds defined by the policy (no capability beyond what's explicitly permitted)

## Bounded Reason Codes

Agents communicate decisions using a FIXED vocabulary:

| Code | Meaning |
|---|---|
| `signal_generated` | Risk pattern detected |
| `signal_below_threshold` | Confidence too low for action |
| `signal_queued_review` | Above threshold, awaiting human |
| `signal_approved` | Human approved escalation |
| `signal_rejected` | Human rejected signal |
| `policy_refused` | Policy verification failed, refusing to act |
| `data_refused` | Provenance verification failed |

## Audit Trail

Every agent action produces an audit entry:
```json
{
  "timestamp": "2026-06-06T22:01:33Z",
  "agent": "claims_anomaly",
  "action": "signal_generated",
  "provider_id": "canonical_id_hash",
  "signal_type": "billing_after_exclusion",
  "confidence": 0.92,
  "policy_version": 2,
  "data_sources": ["leie_updated.aion", "nppes_deactivated.aion"]
}
```
