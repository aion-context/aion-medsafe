# AION-MEDSAFE → aion-context Integration Architecture

## What aion-context Is

`aion-context` is a Rust crate (1.0.0, MIT/Apache-2.0) that wraps any byte payload in a **tamper-evident, hash-chained, cryptographically-signed** container (`.aion` file). It provides:

- **Ed25519 + BLAKE3** signature chain per version
- **K-of-N multisig** for multi-party approval (RFC-0021)
- **Post-quantum hybrid** (ML-DSA-65) for future-proofing (RFC-0027)
- **External artifact manifests** (RFC-0022) — bind large external files by BLAKE3 hash
- **Key registry** with rotation/revocation and epoch tracking (RFC-0028)
- **Sealed releases** with SLSA v1.1 provenance (RFC-0032)
- **Transparency log** (RFC-0025, RFC 6962-compatible)
- **Zero-copy parsing**, offline-first, zero panics (Tiger Style)

**Core principle:** Policy and governance rules live outside the model, are versioned + signed, and verifiable by any auditor without trusting any external service.

---

## Why This Fits AION-MEDSAFE Perfectly

AION-MEDSAFE deals with **evidentiary data that must be provably authentic**. When we flag a provider as "excluded but still billing," that accusation must rest on:

1. **Provenance** — Which exact data file did we base this on?
2. **Integrity** — Has anyone altered the data since we ingested it?
3. **Chain of custody** — When was the data ingested? Who approved it?
4. **Auditability** — Can a court/regulator independently verify our evidence?

This is exactly what `.aion` files provide. Here's how every piece of our pipeline maps:

---

## Integration Map

### 1. Data Source Provenance (`.aion` manifest per snapshot)

Every time our pipeline ingests a bulk file, we create an `.aion`-sealed manifest:

```
Pipeline downloads LEIE UPDATED.csv
  → BLAKE3 hash of raw file
  → Create ArtifactManifest with entry: ("LEIE_UPDATED_2026-06.csv", size, hash)
  → sign_manifest with pipeline's signing key
  → Store as: data/provenance/leie_updated_2026-06.aion
```

**Result:** At any future date, anyone with our registry can verify:
- The exact file we ingested
- When we ingested it
- That no one tampered with it after ingestion

```rust
use aion_context::manifest::{ArtifactManifestBuilder, sign_manifest};
use aion_context::crypto::SigningKey;
use aion_context::types::AuthorId;

// Pipeline ingestion step:
let raw_bytes = std::fs::read("data/raw/UPDATED.csv")?;
let mut builder = ArtifactManifestBuilder::new();
builder.add("LEIE_UPDATED_2026-06.csv", &raw_bytes);
let manifest = builder.build();

let pipeline_author = AuthorId::new(80001); // "medsafe-pipeline-ingestion"
let sig = sign_manifest(&manifest, pipeline_author, &pipeline_key);
// manifest.manifest_id is the BLAKE3 of the canonical manifest → content-addressed
```

### 2. Exclusion Rules as Signed Policy (`.aion` policy file)

Our detection logic — "what constitutes a risk signal" — should be a **signed policy** that the system verifies on every run:

```yaml
# medsafe_detection_policy.yaml (wrapped in .aion)
version: "2.0"
effective_date: "2026-06-06"

risk_signals:
  federal_state_mismatch:
    severity: 0.8
    description: "Provider on federal LEIE but not on state exclusion list"
    requires_human_review: true

  active_npi_while_excluded:
    severity: 0.9
    description: "Provider excluded but NPI status still active in NPPES"
    requires_human_review: true

  billing_after_exclusion:
    severity: 1.0
    description: "Claims submitted after exclusion effective date"
    requires_human_review: true
    escalation: "immediate"

  re_exclusion:
    severity: 0.7
    description: "Provider reinstated then excluded again"
    requires_human_review: true

thresholds:
  minimum_confidence_for_alert: 0.75
  maximum_days_lookback: 1825  # 5 years

jurisdictions:
  primary: "HI"
  scope: "national"  # compute nationally, focus locally
```

```rust
// Before generating any risk signals, verify the policy is untampered:
let report = verify_file(Path::new("policy/medsafe_detection.aion"), &registry)?;
if !report.is_valid {
    // REFUSE to generate risk signals — policy has been tampered with
    return Err(MedsafeError::PolicyIntegrityFailed(report.errors));
}
let rules = show_current_rules(Path::new("policy/medsafe_detection.aion"))?;
let policy = DetectionPolicy::parse(&rules)?;
// Now generate signals using the verified policy
```

**Why this matters:** If our system ever produces evidence used in a fraud prosecution, the defense attorney's first question is "how do we know your detection rules weren't modified after the fact?" The `.aion` chain answers that conclusively.

### 3. Trust Graph Export as Sealed Release (RFC-0032)

Our `TrustGraphExport` (the pipeline's final output) becomes a **sealed release**:

```
aion release seal \
  --artifact trust_graph_export_2026-06.ndjson \
  --registry medsafe.registry.json \
  --author 80001 \
  --slsa-builder "medsafe-pipeline/v0.1.0"
```

This produces:
- SLSA v1.1 Statement proving what built it
- DSSE-wrapped envelope for sigstore-compatible verification
- OCI manifest for registry distribution

### 4. Lifecycle Event Store with Hash Chain

Our `ExclusionEvent` stream maps directly to aion-context's **audit trail**:

```
Event: Provider X excluded   → aion commit (v1)
Event: Provider X reinstated → aion commit (v2)
Event: Provider X re-excluded → aion commit (v3)
```

Each commit to the `.aion` file for a provider's lifecycle is:
- Signed by the pipeline's key
- Hash-chained to the previous event
- Timestamped
- Non-repudiable

### 5. Multi-Party Approval for High-Severity Signals

Using RFC-0021 K-of-N multisig:

```
Risk signal: "Provider billing $2M after exclusion"
  → Requires 2-of-3 approval before escalation:
    - Author 80001: Pipeline automated detection (auto-signs)
    - Author 80002: Senior analyst review (manual sign)
    - Author 80003: Legal counsel approval (manual sign)
```

Only when the quorum threshold is met does the signal advance to enforcement.

---

## Project Structure (Converted to .aion)

```
aion-medsafe/
├── .aion/                          # aion-context configuration
│   ├── medsafe.registry.json       # Key registry (who can sign what)
│   └── keys/                       # Signing keys (or OS keyring ref)
│
├── policy/                         # Signed policy files
│   ├── detection_rules.aion        # What constitutes a risk signal
│   ├── escalation_policy.aion      # When to escalate, to whom
│   └── data_retention.aion         # How long we keep what
│
├── provenance/                     # Data source attestations
│   ├── leie_updated_2026-06.aion   # Manifest for LEIE CSV snapshot
│   ├── nppes_deact_2026-05.aion    # Manifest for NPPES deactivated
│   ├── hawaii_medquest_2026-04.aion # Manifest for HI PDF
│   └── ...
│
├── pipeline/                       # The Python pipeline (current code)
│   ├── src/
│   └── data/
│
└── system/                         # The Rust system (future)
    ├── Cargo.toml                  # depends on aion-context = "1.0"
    ├── src/
    │   ├── main.rs
    │   ├── graph.rs                # Provider Trust Graph
    │   ├── signals.rs              # Risk signal computation
    │   ├── policy.rs               # Policy loading (via aion verify)
    │   ├── provenance.rs           # Artifact verification
    │   └── export.rs               # Sealed release generation
    └── tests/
```

---

## AuthorId Allocation

| AuthorId Range | Role | Signs What |
|---|---|---|
| 80001–80009 | Pipeline automation | Data ingestion manifests, routine commits |
| 80010–80019 | Analyst operators | Policy updates, manual review approvals |
| 80020–80029 | Legal/compliance | Escalation approvals, enforcement actions |
| 80030–80039 | System admin | Key rotations, registry updates |

---

## What Changes in Our Pipeline (Immediate)

1. **After every bulk download** → compute BLAKE3 hash, emit manifest entry
2. **Before every cross-reference** → verify source manifests
3. **Detection rules** → move from code to signed `.aion` policy file
4. **Output** → seal as SLSA-attested release

The Python pipeline becomes the **data preparation** layer. The Rust system (which already uses aion-context as a library dependency) becomes the **trust computation** layer with cryptographic guarantees at every boundary.

---

## Implementation Priority

| Phase | What | Effort |
|---|---|---|
| **Phase 1** | `cargo add aion-context` to Rust system, implement provenance manifests for each bulk download | 1 week |
| **Phase 2** | Move detection rules to `.aion` policy file, verify before signal generation | 2-3 days |
| **Phase 3** | Sealed release for Trust Graph exports | 2-3 days |
| **Phase 4** | Lifecycle event store with per-provider `.aion` chains | 1 week |
| **Phase 5** | K-of-N multisig for escalation workflow | 1 week |

---

## The Payoff

When Hawaii's Medicaid Fraud Strike Force asks "how do we know your data is reliable?":

1. **Here's the exact LEIE file we ingested** → BLAKE3 hash matches what HHS-OIG published
2. **Here's proof it wasn't tampered with** → Signed manifest, hash chain intact
3. **Here's the detection rule that flagged this provider** → Signed policy, versioned, auditable
4. **Here's when we detected it** → Timestamped, signed commit in the event chain
5. **Here's who approved the escalation** → 2-of-3 multisig quorum met

No other system in this space provides this level of evidentiary rigor. The `.aion` file format IS the chain of custody.
