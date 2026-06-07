"""Unified Data Schema for the AION-MEDSAFE Rust System.

This is the handoff contract. The Python pipeline produces these structures;
the Rust system consumes them to build the Provider Trust Graph.

Design principles:
- Every entity gets a stable canonical ID (deterministic from available identifiers)
- All source data is preserved as provenance (never lost in normalization)
- The graph is built from Entities + Events + Signals
- Jurisdictional filtering is a query-time concern, not an ingestion-time concern
- National scope, local focus

The Rust system will build:
  Provider Entity Node → connected to → Exclusion Events
                       → connected to → Identity Claims (names, addresses, NPIs)
                       → connected to → Risk Signals (computed)
                       → connected to → Other Provider Entities (via shared attributes)
"""

from __future__ import annotations

from datetime import datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field


# ─── Enums ────────────────────────────────────────────────────────────────────


class EntityType(StrEnum):
    INDIVIDUAL = "individual"
    ORGANIZATION = "organization"
    UNKNOWN = "unknown"


class IdentifierType(StrEnum):
    NPI = "npi"
    UPIN = "upin"
    STATE_MEDICAID_ID = "state_medicaid_id"
    DEA = "dea"
    SSN_LAST4 = "ssn_last4"


class ExclusionAuthority(StrEnum):
    HHS_OIG = "hhs_oig"           # Federal LEIE mandatory/permissive
    STATE_MEDICAID = "state_medicaid"  # State-level (e.g., Hawaii Med-QUEST)
    SAM_GOV = "sam_gov"           # System for Award Management
    STATE_LICENSE = "state_license"    # State licensing board


class ExclusionStatus(StrEnum):
    ACTIVE = "active"
    REINSTATED = "reinstated"
    INDEFINITE = "indefinite"


class RiskSignalType(StrEnum):
    MULTI_STATE_EXCLUSION = "multi_state_exclusion"
    RE_EXCLUSION = "re_exclusion"
    ACTIVE_NPI_WHILE_EXCLUDED = "active_npi_while_excluded"
    BILLING_AFTER_EXCLUSION = "billing_after_exclusion"
    NAME_VARIANT_ACROSS_SOURCES = "name_variant_across_sources"
    FEDERAL_STATE_MISMATCH = "federal_state_mismatch"
    HIGH_RISK_SPECIALTY = "high_risk_specialty"
    GEOGRAPHIC_MOBILITY = "geographic_mobility"


# ─── Identity Claims ──────────────────────────────────────────────────────────


class IdentifierClaim(BaseModel):
    """A specific identifier asserted by a source."""
    identifier_type: IdentifierType
    value: str
    source_id: str
    observed_at: datetime
    source_snapshot_hash: str


class NameClaim(BaseModel):
    """A name asserted by a source. An entity may have many."""
    last_name: str | None = None
    first_name: str | None = None
    middle_name: str | None = None
    business_name: str | None = None
    suffix: str | None = None
    full_name: str  # denormalized for display/search
    source_id: str
    observed_at: datetime


class AddressClaim(BaseModel):
    """An address asserted by a source."""
    address_1: str | None = None
    address_2: str | None = None
    city: str | None = None
    state: str | None = None
    zip_code: str | None = None
    country: str = "US"
    address_purpose: str | None = None  # "mailing", "practice", "home"
    source_id: str
    observed_at: datetime


class TaxonomyClaim(BaseModel):
    """A provider specialty/taxonomy from NPPES or other source."""
    code: str
    description: str | None = None
    is_primary: bool = False
    license_number: str | None = None
    state: str | None = None
    source_id: str
    observed_at: datetime


# ─── Core Entity ──────────────────────────────────────────────────────────────


class ProviderEntity(BaseModel):
    """The canonical provider entity node in the Trust Graph.

    This is the RESOLVED entity after entity resolution runs.
    One ProviderEntity may have been constructed from multiple source records.
    """
    entity_id: str = Field(description="Deterministic ID: NPI if available, else hash of canonical name + state")
    entity_type: EntityType
    canonical_name: str = Field(description="Best-known name for display")
    canonical_state: str | None = Field(default=None, description="Primary state of operation")

    identifiers: list[IdentifierClaim] = Field(default_factory=list)
    names: list[NameClaim] = Field(default_factory=list)
    addresses: list[AddressClaim] = Field(default_factory=list)
    taxonomies: list[TaxonomyClaim] = Field(default_factory=list)

    source_record_ids: list[str] = Field(default_factory=list, description="All source records that contributed to this entity")
    resolution_confidence: float = Field(default=1.0, description="Confidence in entity resolution (0-1)")
    created_at: datetime | None = None
    updated_at: datetime | None = None


# ─── Exclusion Events ─────────────────────────────────────────────────────────


class ExclusionEvent(BaseModel):
    """An exclusion or reinstatement event tied to a ProviderEntity."""
    event_id: str = Field(description="Deterministic: hash of entity_id + authority + date + type")
    entity_id: str = Field(description="FK to ProviderEntity.entity_id")
    authority: ExclusionAuthority
    exclusion_type: str | None = Field(default=None, description="e.g., '1128a1', '1128a3' from LEIE")
    exclusion_date: datetime | None = None
    reinstatement_date: datetime | None = None
    status: ExclusionStatus
    state: str | None = Field(default=None, description="State where exclusion applies (null = all states)")
    waiver_state: str | None = None
    waiver_date: datetime | None = None

    # Provenance
    source_id: str
    source_record_id: str
    source_snapshot_hash: str
    observed_at: datetime

    # Enrichment from source
    general_category: str | None = None  # e.g., "IND- LIC HC SERV PRO"
    specialty: str | None = None  # e.g., "PSYCHOLOGIST"


# ─── Risk Signals ─────────────────────────────────────────────────────────────


class RiskSignal(BaseModel):
    """A computed risk signal attached to a ProviderEntity.

    These are NOT accusations. They are evidence-ranked indicators
    that warrant human review. The system assists, not accuses.
    """
    signal_id: str
    entity_id: str
    signal_type: RiskSignalType
    severity: float = Field(ge=0.0, le=1.0, description="0=informational, 1=critical")
    description: str
    evidence: list[str] = Field(default_factory=list, description="List of source event_ids supporting this signal")
    computed_at: datetime
    requires_human_review: bool = True


# ─── Graph Export Format ──────────────────────────────────────────────────────


class TrustGraphExport(BaseModel):
    """The complete export from the pipeline to the Rust system.

    This is what gets serialized and handed off.
    """
    export_version: str = "1.0.0"
    exported_at: datetime
    pipeline_version: str

    # Counts for validation
    entity_count: int
    exclusion_event_count: int
    risk_signal_count: int

    # The data
    entities: list[ProviderEntity]
    exclusion_events: list[ExclusionEvent]
    risk_signals: list[RiskSignal]

    # Metadata
    sources_ingested: list[str]
    jurisdiction_coverage: list[str] = Field(description="States with data coverage")
    snapshot_hashes: dict[str, str] = Field(default_factory=dict, description="source_id -> latest snapshot hash")
