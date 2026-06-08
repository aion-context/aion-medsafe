# Sealed Evidence Packet (SEP) Specification

**Version:** 0.1 (Draft)
**Status:** Open specification. Reference implementation: AION-MEDSAFE.
**License:** This specification is published under Apache-2.0; anyone may
implement or produce conformant packets.

A Sealed Evidence Packet is a **tamper-evident, independently verifiable dossier**
about a single subject (e.g. a healthcare provider), assembling the risk signals
raised about that subject together with the underlying evidence — each item bound
to the cryptographic hash of its source — and an attestation tying the whole
packet to the signed rules and data it was derived from.

The goal is an interoperable, court-/audit-defensible evidence format: given only
the public key registry, any third party can confirm a packet was built from
exactly the stated sources, under exactly the stated rules, reviewed by exactly
the stated reviewer — offline, indefinitely.

---

## 1. Conformance

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are used per
RFC 2119.

A *producer* is any system that emits packets. A *verifier* is any system that
validates them. This spec is **vendor-neutral**: AION-MEDSAFE is one reference
producer/verifier, not the definition.

**Conformance levels:**

- **L1 — Sealed:** the packet container verifies (§3) and every packet object is
  schema-valid (§5).
- **L2 — Provenanced:** in addition, every evidence item's `source_snapshot_blake3`
  resolves to a sealed source, and the `attestation` digests resolve to the sealed
  policy and graph (§7). L2 is the bar for hearing-grade evidence.

## 2. Design principles (normative intent)

1. **Leads, not verdicts.** A packet is investigative evidence for human review.
   A producer MUST NOT present a packet as an adjudication or accusation.
2. **No autonomous escalation.** Any escalation state in a packet MUST reflect a
   recorded human decision, not an automated one.
3. **Provenance is mandatory.** Every evidence item MUST carry the cryptographic
   digest of its source. Claims without a resolvable source digest are not
   conformant evidence.
4. **Verify, never trust.** A verifier MUST re-derive integrity on every use and
   MUST NOT cache trust decisions.
5. **Bounded vocabularies.** Reason codes and authority labels come from fixed
   vocabularies (§6) so packets are comparable across producers.

## 3. Container & sealing

- A packet collection MUST be serialized as **NDJSON** (one JSON object per line,
  UTF-8): exactly one `packet_run_meta` record (§4) followed by zero or more
  `case_packet` records (§5).
- The NDJSON payload MUST be sealed into a tamper-evident envelope providing four
  guarantees, re-checkable by a verifier:
  1. **Structure** — the envelope parses within declared bounds.
  2. **Integrity** — a content hash proves no byte changed since sealing.
  3. **Hash chain** — version lineage is intact across re-seals.
  4. **Signatures** — signed by an author present in a key registry.
- The reference envelope is an `aion-context` `.aion` file using **BLAKE3**
  (hashing) and **Ed25519** (signing). Producers MAY use another envelope that
  provides the same four guarantees; the envelope algorithm MUST be identified.
- A verifier MUST refuse to treat an unverified envelope's contents as evidence.

## 4. `packet_run_meta` (one per collection)

| Field | Type | Req | Notes |
|---|---|---|---|
| `record` | string | MUST | Constant `"packet_run_meta"` |
| `generated_at` | string (RFC 3339) | MUST | Run timestamp |
| `jurisdiction` | string | SHOULD | e.g. `"HI"`, or `"national"` |
| `flagged_providers` | integer | SHOULD | Subjects with ≥1 queued signal |
| `packets_written` | integer | SHOULD | Packets in this collection |

## 5. `case_packet` object

| Field | Type | Req | Notes |
|---|---|---|---|
| `record` | string | MUST | Constant `"case_packet"` |
| `packet_id` | string (hex) | MUST | Stable id; reference = first 16 bytes of `BLAKE3(entity_id \| policy_version \| generated_at)` |
| `generated_at` | string (RFC 3339) | MUST | |
| `jurisdiction` | string | SHOULD | |
| `entity_id` | string | MUST | Stable subject id within the producer's resolution scope |
| `canonical_name` | string | MUST | Resolved subject name |
| `canonical_state` | string | SHOULD | 2-letter code |
| `npis` | string[] | SHOULD | Associated NPIs (may be empty) |
| `npi_active` | boolean | MAY | NPPES active status, if known |
| `resolution_confidence` | number 0–1 | SHOULD | Entity-resolution confidence |
| `federal_lists` | string[] | SHOULD | Federal lists naming the subject (§6) |
| `on_state_medicaid` | boolean | SHOULD | On a state Medicaid exclusion list |
| `state_license_action` | boolean | SHOULD | Has a state licensing-board adverse action |
| `ownership` | Ownership | MAY | Present only if the subject owns active providers (§5.3) |
| `signals` | Signal[] | MUST | Risk signals (§5.1); MAY be empty |
| `evidence` | EvidenceItem[] | MUST | Underlying evidence (§5.2) |
| `attestation` | Attestation | MUST | Binds packet to its inputs (§5.4) |

Producers MAY add fields; verifiers MUST ignore unknown fields (forward
compatibility).

### 5.1 Signal

| Field | Type | Req | Notes |
|---|---|---|---|
| `signal_id` | string | MUST | Stable id (reference = `BLAKE3(signal_type:entity_id)[..16]`) |
| `signal_type` | string | MUST | Producer-defined type (e.g. `active_npi_while_excluded`) |
| `severity` | number 0–1 | MUST | Policy-assigned severity |
| `confidence` | number 0–1 | MUST | Computed confidence |
| `description` | string | MUST | Human-readable basis |
| `evidence` | string[] | SHOULD | Event ids / references supporting the signal |
| `requires_human_review` | boolean | MUST | Per policy |
| `reason_code` | string | MUST | From the reason-code vocabulary (§6) |
| `calibrated_precision` | number 0–1 | MAY | Earned precision for this `signal_type` from adjudicated outcomes |

Producers SHOULD distinguish **asserted** `confidence` from **earned**
`calibrated_precision`; verifiers and reviewers SHOULD weight the latter when
present.

### 5.2 EvidenceItem

| Field | Type | Req | Notes |
|---|---|---|---|
| `event_id` | string | MUST | Stable id for this evidence event |
| `authority` | string | MUST | Issuing authority label (§6) |
| `source_id` | string | MUST | Source dataset id (e.g. `hhs_oig_leie`, `sam_gov`) |
| `source_record_id` | string | SHOULD | Record id within the source |
| `source_snapshot_blake3` | string (hex) | **MUST** | Digest of the sealed source the item came from — the provenance anchor |
| `status` | string | SHOULD | e.g. `Active`, `Reinstated`, `Indefinite` |
| `state` | string | MAY | 2-letter code |
| `exclusion_date` | string (RFC 3339) | MAY | |
| `reinstatement_date` | string (RFC 3339) | MAY | |
| `basis` | string | MAY | Cause/agency (e.g. SAM exclusion type + excluding agency) |

`source_snapshot_blake3` is the heart of the format: it is what makes an evidence
claim *checkable* against the original government data.

### 5.3 Ownership (optional)

| Field | Type | Req | Notes |
|---|---|---|---|
| `confidence` | number 0–1 | MUST | Match confidence |
| `state_corroborated` | boolean | MUST | Whether owner state corroborated the match |
| `owned_count` | integer | MUST | Active providers owned |
| `owned_sample` | OwnedProvider[] | SHOULD | Bounded sample |

`OwnedProvider`: `{ provider_org_name: string, provider_type: string,
role?: string, ownership_pct?: number }`.

Producers MUST label low-precision (e.g. name-only) ownership matches as such in
`confidence`/description and MUST NOT present them as confirmed.

### 5.4 Attestation

| Field | Type | Req | Notes |
|---|---|---|---|
| `graph_manifest_blake3` | string (hex) | MUST | Digest of the sealed subject graph |
| `policy_version` | string | MUST | Version of the detection policy applied |
| `policy_manifest_blake3` | string (hex) | MUST | Digest of the sealed policy |
| `registry` | string | SHOULD | Locator for the public-key registry |

## 6. Bounded vocabularies

**Reason codes** (extensible; these are reserved):
`signal_queued_review`, `signal_below_threshold`, `signal_approved`,
`signal_rejected`, `policy_refused`, `data_refused`.

**Authority labels** (reference set): `HHS-OIG (LEIE)`, `SAM.gov`,
`State Medicaid`, `State licensing board`. Producers MAY extend; the label MUST
identify the issuing oversight body.

## 7. Verification procedure

A verifier, given a packet envelope and the public-key registry, MUST:

1. Verify the envelope's four guarantees (§3). Reject on any failure.
2. Parse the NDJSON; validate each record against §4/§5. Reject malformed records.
3. **For L2:** for each `case_packet`:
   a. For each `EvidenceItem`, confirm `source_snapshot_blake3` matches a sealed
      source manifest the verifier can independently verify.
   b. Confirm `attestation.policy_manifest_blake3` and `graph_manifest_blake3`
      match the sealed policy and graph (also independently verifiable).
4. Report per-packet conformance level (L1 / L2) and any failures.

A verifier MUST treat a packet whose envelope fails as **non-evidence**, and
SHOULD surface exactly which guarantee failed.

## 8. Versioning & extensibility

- This document is `SEP/0.1`. Breaking changes increment the major version.
- Producers MUST NOT remove or repurpose defined fields within a major version.
- Unknown fields and unknown reason codes/authority labels MUST be ignored (not
  rejected) by verifiers, to allow additive evolution.

## 9. Non-goals

- SEP does not define *how* signals are computed (that is producer/policy
  specific) — only how the resulting evidence is structured, sealed, and verified.
- SEP does not standardize scoring models or thresholds.
- SEP is not a transport protocol; packets are files.

## Appendix A — Example `case_packet` (abridged)

```json
{
  "record": "case_packet",
  "packet_id": "e4ccae1ab854d129",
  "generated_at": "2026-06-07T07:31:20Z",
  "jurisdiction": "HI",
  "entity_id": "1043470370",
  "canonical_name": "YANG SUNG S.",
  "canonical_state": "HI",
  "npis": ["1043470370"],
  "npi_active": true,
  "resolution_confidence": 1.0,
  "federal_lists": ["HHS-OIG (LEIE)"],
  "on_state_medicaid": true,
  "state_license_action": false,
  "signals": [
    {
      "signal_id": "f365c227c2307368",
      "signal_type": "active_npi_while_excluded",
      "severity": 0.90,
      "confidence": 0.85,
      "description": "NPI active in NPPES while under active exclusion",
      "evidence": ["evt_a", "evt_b"],
      "requires_human_review": true,
      "reason_code": "signal_queued_review",
      "calibrated_precision": 0.67
    }
  ],
  "evidence": [
    {
      "event_id": "evt_a",
      "authority": "HHS-OIG (LEIE)",
      "source_id": "hhs_oig_leie",
      "source_record_id": "82402",
      "source_snapshot_blake3": "9aee29ee97e21ae1f6f0b1c4349ef48bbd651ddb2409d3128b068c171f72acb6",
      "status": "Active",
      "state": "HI",
      "exclusion_date": "2021-08-19T00:00:00Z"
    },
    {
      "event_id": "evt_b",
      "authority": "State Medicaid",
      "source_id": "hawaii_medquest_exclusions",
      "source_record_id": "644014",
      "source_snapshot_blake3": "f2ba8681e6aa1db3884811fa519da580f11cd284caf55fe3edf7875d5dcfc3ce",
      "status": "Indefinite",
      "state": "HI",
      "exclusion_date": "2021-08-19T00:00:00Z"
    }
  ],
  "attestation": {
    "graph_manifest_blake3": "2142c9d2d96621cf374620a162b12ef36ac07b329134dcf05fc6f264f01403c3",
    "policy_version": "1",
    "policy_manifest_blake3": "b7794460ee4ec3fcc4312c7bf16d01c8b7ee24dd040fb4e488d666f730773b9b",
    "registry": ".aion/medsafe.registry.json"
  }
}
```

---

*SEP/0.1 is a draft open specification. Feedback and independent implementations
are welcome. The AION-MEDSAFE reference implementation produces and verifies
conformant packets today.*
