from aion_medsafe_pipeline.models import (
    AccessMethod,
    PublicDataSource,
    RefreshCadence,
    SourcePriority,
)

PUBLIC_DATA_SOURCES: tuple[PublicDataSource, ...] = (
    PublicDataSource(
        source_id="cms_nppes_npi_registry",
        name="NPPES NPI Registry",
        owner="Centers for Medicare & Medicaid Services",
        url="https://npiregistry.cms.hhs.gov/api-page",
        access_method=AccessMethod.API,
        refresh_cadence=RefreshCadence.WEEKLY,
        priority=SourcePriority.MVP,
        expected_entities=["provider", "organization", "taxonomy", "address"],
        notes="Primary public provider identity backbone for NPI-linked records.",
    ),
    PublicDataSource(
        source_id="cms_nppes_downloads",
        name="NPPES Downloadable Files",
        owner="Centers for Medicare & Medicaid Services",
        url="https://download.cms.gov/nppes/NPI_Files.html",
        access_method=AccessMethod.DOWNLOAD,
        refresh_cadence=RefreshCadence.WEEKLY,
        priority=SourcePriority.MVP,
        expected_entities=["provider", "organization", "taxonomy", "address"],
        notes="Bulk NPI file source for reproducible snapshots and backfills.",
    ),
    PublicDataSource(
        source_id="hhs_oig_leie",
        name="HHS-OIG LEIE Exclusions",
        owner="U.S. Department of Health and Human Services Office of Inspector General",
        url="https://oig.hhs.gov/exclusions/leie-database-supplement-downloads/",
        access_method=AccessMethod.DOWNLOAD,
        refresh_cadence=RefreshCadence.MONTHLY,
        priority=SourcePriority.MVP,
        expected_entities=["excluded_person", "excluded_entity", "reinstatement"],
        notes="Federal healthcare exclusion source of truth for payment eligibility screening.",
    ),
    PublicDataSource(
        source_id="hawaii_medquest_exclusions",
        name="Hawaii Med-QUEST Provider Exclusion/Reinstatement List",
        owner="Hawaii Med-QUEST Division",
        url="https://medquest.hawaii.gov/en/plans-providers/provider-exclusion-reinstatement-list.html",
        access_method=AccessMethod.DOWNLOAD,
        refresh_cadence=RefreshCadence.MONTHLY,
        priority=SourcePriority.MVP,
        expected_entities=["excluded_provider", "reinstated_provider"],
        notes="Hawaii-specific Medicaid exclusion and reinstatement signal.",
    ),
    PublicDataSource(
        source_id="sam_gov_exclusions",
        name="SAM.gov Exclusions",
        owner="U.S. General Services Administration",
        url="https://open.gsa.gov/api/exclusions-api/",
        access_method=AccessMethod.API,
        refresh_cadence=RefreshCadence.DAILY,
        priority=SourcePriority.MVP,
        expected_entities=["excluded_entity", "excluded_individual"],
        notes="Federal debarment and exclusion source for broader entity risk screening.",
    ),
    PublicDataSource(
        source_id="cms_public_provider_enrollment",
        name="Medicare Fee-For-Service Public Provider Enrollment Data",
        owner="Centers for Medicare & Medicaid Services",
        url="https://data.cms.gov/provider-characteristics/medicare-provider-supplier-enrollment/medicare-fee-for-service-public-provider-enrollment",
        access_method=AccessMethod.DOWNLOAD,
        refresh_cadence=RefreshCadence.QUARTERLY,
        priority=SourcePriority.PHASE_2,
        expected_entities=["provider_enrollment", "supplier", "organization", "practice_location"],
        notes="PECOS-derived public enrollment data for graph enrichment and identity reconciliation.",
    ),
)


def list_sources() -> tuple[PublicDataSource, ...]:
    return PUBLIC_DATA_SOURCES
