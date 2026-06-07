"""Bulk Data Source Registry — Raw Files Only, No APIs.

Design principle: Always prefer the rawest form of government data.
- CSV/flat files over APIs
- Full dumps over incremental queries
- We do our own transformations
- No rate limits, no dependencies on third-party uptime
- Deterministic: same input file → same output every time

Every source listed here is a direct download URL to a bulk flat file
published by a government entity. No authentication required for public data.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum


class FileFormat(StrEnum):
    CSV = "csv"
    ZIP_CSV = "zip_csv"       # ZIP containing CSV(s)
    EXCEL = "excel"
    PDF = "pdf"
    NDJSON = "ndjson"
    PIPE_DELIMITED = "pipe_delimited"


class RefreshCadence(StrEnum):
    DAILY = "daily"
    WEEKLY = "weekly"
    MONTHLY = "monthly"
    QUARTERLY = "quarterly"
    AS_UPDATED = "as_updated"


class DataScope(StrEnum):
    NATIONAL = "national"
    STATE = "state"


@dataclass
class BulkSource:
    """A bulk downloadable data source from a government entity."""
    source_id: str
    name: str
    description: str
    url: str
    format: FileFormat
    refresh: RefreshCadence
    scope: DataScope
    publisher: str
    records_expected: str  # human-readable estimate
    size_estimate: str     # human-readable file size estimate
    notes: list[str] = field(default_factory=list)
    requires_auth: bool = False
    state: str | None = None  # for state-scoped sources


# ─── TIER 1: Core Exclusion Lists (Bulk CSV/Flat Files) ──────────────────────


LEIE_UPDATED = BulkSource(
    source_id="hhs_oig_leie_updated",
    name="HHS-OIG LEIE — Full Exclusion List",
    description="Complete list of all currently excluded individuals and entities. "
                "This is the master federal exclusion file.",
    url="https://oig.hhs.gov/exclusions/downloadables/UPDATED.csv",
    format=FileFormat.CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="HHS Office of Inspector General",
    records_expected="~83,000 records",
    size_estimate="~15 MB",
    notes=[
        "Columns: LASTNAME, FIRSTNAME, MIDNAME, BUSNAME, GENERAL, SPECIALTY, "
        "UPIN, NPI, DOB, ADDRESS, CITY, STATE, ZIP, EXCLTYPE, EXCLDATE, "
        "REINDATE, WAIVERDATE, WAIVERSTATE",
        "NPI populated for ~10% of records",
        "Encoding: UTF-8, standard CSV",
        "Updated on the 15th-20th of each month",
    ],
)

LEIE_REINSTATEMENTS = BulkSource(
    source_id="hhs_oig_leie_reinstatements",
    name="HHS-OIG LEIE — Reinstatement List",
    description="Providers reinstated to participation after exclusion period.",
    url="https://oig.hhs.gov/exclusions/downloadables/REIN.csv",
    format=FileFormat.CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="HHS Office of Inspector General",
    records_expected="~40,000 records",
    size_estimate="~5 MB",
    notes=["Same column schema as UPDATED.csv", "Includes reinstatement dates"],
)

SAM_EXCLUSIONS = BulkSource(
    source_id="sam_gov_exclusions",
    name="SAM.gov Exclusions Public Extract",
    description="Federal government-wide exclusion/debarment records. "
                "Covers all federal contracts, grants, and benefits programs.",
    url="https://sam.gov/data-services/Exclusions/Public%20V2?privacy=Public",
    format=FileFormat.CSV,
    refresh=RefreshCadence.DAILY,
    scope=DataScope.NATIONAL,
    publisher="General Services Administration (GSA)",
    records_expected="~150,000 active exclusion records",
    size_estimate="~50 MB",
    notes=[
        "Covers debarments, suspensions, proposed debarments, and statutory exclusions",
        "Broader than healthcare: includes defense contractors, grant recipients",
        "Cross-reference with LEIE identifies providers excluded from BOTH programs",
        "Public extract available without authentication at sam.gov/data-services",
        "Layout spec: open.gsa.gov/api/sam-entity-extracts-api/v1/SAM_Exclusions_Public_Extract_Layout.pdf",
    ],
)


# ─── TIER 2: Provider Identity (Bulk Flat Files) ─────────────────────────────


NPPES_FULL = BulkSource(
    source_id="cms_nppes_full",
    name="NPPES Full NPI Data Dissemination File (V2)",
    description="Complete registry of all National Provider Identifiers. "
                "Contains every healthcare provider ever issued an NPI.",
    url="https://download.cms.gov/nppes/NPPES_Data_Dissemination_May_2026_V2.zip",
    format=FileFormat.ZIP_CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="CMS (Centers for Medicare & Medicaid Services)",
    records_expected="~8 million NPI records",
    size_estimate="~9 GB compressed, ~40 GB uncompressed",
    notes=[
        "THIS IS THE RAW FILE. No API needed. Contains everything the API returns and more.",
        "Includes: NPI, entity type, name, credentials, taxonomy codes, addresses, "
        "enumeration date, deactivation date, reactivation date",
        "Also includes 3 reference files: Other Names, Practice Locations, Endpoints",
        "We only need to extract rows matching our 8,429 excluded NPIs — not all 8M",
        "Can also be used to find deactivated NPIs (providers hiding from exclusion)",
        "V2 format: Extended field lengths for names, UTF-8, standard CSV in ZIP",
    ],
)

NPPES_WEEKLY = BulkSource(
    source_id="cms_nppes_weekly",
    name="NPPES Weekly Incremental Update (V2)",
    description="Weekly delta file for NPI changes since last monthly full dump.",
    url="https://download.cms.gov/nppes/NPPES_Data_Dissemination_052526_053126_Weekly_V2.zip",
    format=FileFormat.ZIP_CSV,
    refresh=RefreshCadence.WEEKLY,
    scope=DataScope.NATIONAL,
    publisher="CMS",
    records_expected="~50,000-100,000 changed records per week",
    size_estimate="~100 MB compressed",
    notes=[
        "Use to keep NPI data fresh between monthly full loads",
        "Same schema as monthly file",
    ],
)

NPPES_DEACTIVATED = BulkSource(
    source_id="cms_nppes_deactivated",
    name="NPPES Deactivated NPI Report (V2)",
    description="NPIs that have been deactivated. Critical for detecting "
                "providers who deactivated after exclusion.",
    url="https://download.cms.gov/nppes/NPPES_Deactivated_NPI_Report_051126_V2.zip",
    format=FileFormat.ZIP_CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="CMS",
    records_expected="~1 million deactivated NPIs",
    size_estimate="~50 MB compressed",
    notes=[
        "Contains deactivation dates",
        "Cross-reference with LEIE: did they deactivate to hide from detection?",
        "Red flag: NPI deactivated shortly after exclusion date",
    ],
)


# ─── TIER 3: Medicare Provider Enrollment & Activity ─────────────────────────


MEDICARE_ENROLLMENT = BulkSource(
    source_id="cms_medicare_enrollment",
    name="Medicare Fee-For-Service Public Provider Enrollment",
    description="All providers actively approved to bill Medicare, from PECOS.",
    url="https://data.cms.gov/provider-characteristics/medicare-provider-supplier-enrollment/medicare-fee-for-service-public-provider-enrollment",
    format=FileFormat.CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="CMS",
    records_expected="~2 million enrollment records",
    size_estimate="~500 MB",
    notes=[
        "Download as CSV from data.cms.gov (no API key required)",
        "Critical cross-reference: provider on LEIE but still enrolled = HIGH RISK",
        "Contains: NPI, enrollment type, specialty, state, enrollment date",
        "Direct download link changes monthly — scrape from data.cms.gov portal",
    ],
)

MEDICARE_OPT_OUT = BulkSource(
    source_id="cms_medicare_opt_out",
    name="Medicare Provider Opt-Out Affidavits",
    description="Providers who opted out of Medicare. May indicate providers "
                "trying to avoid Medicare scrutiny while billing Medicaid.",
    url="https://data.cms.gov/provider-characteristics/medicare-provider-supplier-enrollment/opt-out-affidavits",
    format=FileFormat.CSV,
    refresh=RefreshCadence.MONTHLY,
    scope=DataScope.NATIONAL,
    publisher="CMS",
    records_expected="~40,000 opt-out records",
    size_estimate="~5 MB",
    notes=[
        "Cross-reference: opted out of Medicare but still billing Medicaid?",
        "Contains: NPI, name, specialty, opt-out effective/end dates",
        "Downloadable as CSV from data.cms.gov portal",
    ],
)


# ─── TIER 4: State-Level Sources (Hawaii First, Then Expand) ────────────────


HAWAII_MEDQUEST_EXCLUSIONS = BulkSource(
    source_id="hawaii_medquest_exclusions",
    name="Hawaii Med-QUEST Provider Exclusion/Reinstatement List",
    description="Hawaii's state-maintained exclusion list. Published as PDF "
                "by Med-QUEST division of DHS.",
    url="https://medquest.hawaii.gov/content/dam/formsanddocuments/plans-and-providers/"
        "provider-exclusion-reinstatement-list/med-prov-excel-rein-list-2026/"
        "Med%20Prov%20Excl-Rein%20List-UPDATED-04.17.2026.pdf",
    format=FileFormat.PDF,
    refresh=RefreshCadence.AS_UPDATED,
    scope=DataScope.STATE,
    state="HI",
    publisher="Hawaii Department of Human Services / Med-QUEST Division",
    records_expected="~200 records",
    size_estimate="~200 KB",
    notes=[
        "PDF format (no CSV available from Hawaii)",
        "Contains: Name, Exclusion Date, Reinstatement Date, Status",
        "Requires PDF parsing with confidence scoring and drift detection",
        "URL changes with each update — need to scrape parent page for latest",
        "37 providers on federal LEIE for HI not on this state list",
    ],
)


# ─── Source Registry ──────────────────────────────────────────────────────────


BULK_SOURCES: list[BulkSource] = [
    # Tier 1: Exclusion lists
    LEIE_UPDATED,
    LEIE_REINSTATEMENTS,
    SAM_EXCLUSIONS,
    # Tier 2: Provider identity
    NPPES_FULL,
    NPPES_WEEKLY,
    NPPES_DEACTIVATED,
    # Tier 3: Medicare enrollment/activity
    MEDICARE_ENROLLMENT,
    MEDICARE_OPT_OUT,
    # Tier 4: State exclusion lists
    HAWAII_MEDQUEST_EXCLUSIONS,
]


def get_source(source_id: str) -> BulkSource | None:
    for s in BULK_SOURCES:
        if s.source_id == source_id:
            return s
    return None


def list_sources_by_tier() -> dict[str, list[BulkSource]]:
    return {
        "tier1_exclusions": [LEIE_UPDATED, LEIE_REINSTATEMENTS, SAM_EXCLUSIONS],
        "tier2_identity": [NPPES_FULL, NPPES_WEEKLY, NPPES_DEACTIVATED],
        "tier3_enrollment": [MEDICARE_ENROLLMENT, MEDICARE_OPT_OUT],
        "tier4_state": [HAWAII_MEDQUEST_EXCLUSIONS],
    }


# ─── Ingestion Priority Order ────────────────────────────────────────────────

INGESTION_ORDER = """
Priority order for initial ingestion:

1. LEIE_UPDATED (83K records, CSV, immediate)
   → Gives us every currently excluded provider nationally

2. LEIE_REINSTATEMENTS (40K records, CSV, immediate)
   → Completes the lifecycle picture (who was excluded and came back)

3. NPPES_FULL (8M records, but we only extract ~8,600 matching NPIs)
   → Enriches excluded providers with full identity, specialty, addresses
   → No API call needed — we have the entire NPI registry locally

4. SAM_EXCLUSIONS (150K records, CSV, immediate)
   → Federal debarment cross-reference
   → Providers excluded from BOTH healthcare AND federal contracts = severe

5. MEDICARE_ENROLLMENT (2M records, CSV)
   → Critical: "Is this excluded provider STILL enrolled to bill Medicare?"
   → This is the smoking gun for active fraud

6. NPPES_DEACTIVATED (1M records, CSV)
   → Detect providers who deactivated NPI to hide from scrutiny

7. MEDICARE_OPT_OUT (40K records, CSV)
   → Detect providers gaming the system across programs

8. HAWAII_MEDQUEST_EXCLUSIONS (200 records, PDF)
   → State-level validation layer
   → Expand to other states as needed

Total estimated raw data: ~12 million records
After filtering to relevant entities: ~150K-200K records in the graph
"""
