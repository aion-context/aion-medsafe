# Domain Knowledge — Medicaid Fraud Intelligence

## Key Terminology

| Term | Meaning |
|---|---|
| **LEIE** | List of Excluded Individuals/Entities (HHS-OIG maintained) |
| **NPI** | National Provider Identifier (10-digit, unique per provider) |
| **NPPES** | National Plan & Provider Enumeration System (NPI registry) |
| **SAM.gov** | System for Award Management (federal exclusions) |
| **MFCU** | Medicaid Fraud Control Unit (state-level) |
| **CMS** | Centers for Medicare & Medicaid Services |
| **OIG** | Office of Inspector General (HHS) |
| **Med-QUEST** | Hawaii's Medicaid division |
| **FWA** | Fraud, Waste, and Abuse |
| **EDI** | Electronic Data Interchange (claims format) |
| **MMIS** | Medicaid Management Information System |

## Exclusion Types (OIG Section References)

| Section | Meaning | Mandatory? |
|---|---|---|
| 1128(a)(1) | Conviction of program-related crimes | Yes |
| 1128(a)(2) | Conviction of patient abuse/neglect | Yes |
| 1128(a)(3) | Felony conviction for healthcare fraud | Yes |
| 1128(a)(4) | Felony conviction for controlled substance | Yes |
| 1128(b)(1)–(16) | Permissive exclusions (various) | No |

## Fraud Patterns This System Detects

### Active NPI While Excluded
Provider is on LEIE but NPI remains active in NPPES. May indicate they're still billing through another entity.

### Billing After Exclusion
Claims submitted after the exclusion effective date. Clear violation, high confidence.

### Federal-State Mismatch
Provider on federal LEIE but NOT on state exclusion list. May indicate the state hasn't acted yet, or information lag.

### Re-Exclusion Pattern
Provider was excluded → reinstated → excluded again. Indicates persistent problematic behavior.

### NPI Deactivation After Exclusion
Provider deactivates NPI after being excluded. May indicate they're trying to "disappear" before reappearing under a new entity.

## Hawaii-Specific Context

- **Med-QUEST** manages Medicaid for Hawaii
- State exclusion list is published as PDF (no machine-readable format)
- ~200 providers on state exclusion list
- Hawaii has 5 managed care health plans for Medicaid
- Population served: ~400,000 Medicaid beneficiaries
- Key risk areas: behavioral health, home health, personal care services

## Legal Constraints

- **Due Process:** Providers have appeal rights before exclusion takes effect
- **Reinstatement:** Providers can apply for reinstatement after minimum exclusion period
- **Overpayment Recovery:** If provider billed while excluded, recovery must follow federal procedure
- **Chain of Custody:** All evidence must be sealed and timestamped for legal proceedings
- **No Self-Incrimination:** System outputs are investigative leads, not accusations
- **42 CFR Part 455:** Federal regulations governing Medicaid fraud detection

## Data Refresh Cadence

| Source | Update Frequency | Our Ingestion Target |
|---|---|---|
| LEIE Master | Monthly (full) | Monthly |
| LEIE Supplements | Monthly (incremental) | Monthly |
| NPPES Full | Weekly | Quarterly |
| NPPES Deactivated | Monthly | Monthly |
| SAM.gov | Daily | Monthly |
| Hawaii Med-QUEST | Irregular | As published |
