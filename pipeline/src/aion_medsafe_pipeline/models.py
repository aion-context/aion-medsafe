from datetime import UTC, datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field, HttpUrl


class AccessMethod(StrEnum):
    API = "api"
    DOWNLOAD = "download"
    SCRAPE = "scrape"
    MANUAL = "manual"


class RefreshCadence(StrEnum):
    DAILY = "daily"
    WEEKLY = "weekly"
    MONTHLY = "monthly"
    QUARTERLY = "quarterly"
    AD_HOC = "ad_hoc"


class SourcePriority(StrEnum):
    MVP = "mvp"
    PHASE_2 = "phase_2"
    PHASE_3 = "phase_3"


class PublicDataSource(BaseModel):
    source_id: str
    name: str
    owner: str
    url: HttpUrl
    access_method: AccessMethod
    refresh_cadence: RefreshCadence
    priority: SourcePriority
    expected_entities: list[str]
    notes: str


class SourceSnapshot(BaseModel):
    source_id: str
    fetched_at: datetime = Field(default_factory=lambda: datetime.now(UTC))
    source_url: HttpUrl
    content_sha256: str
    record_count: int | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class NormalizedProviderIdentity(BaseModel):
    source_id: str
    source_record_id: str
    observed_at: datetime
    npi: str | None = None
    legal_name: str | None = None
    organization_name: str | None = None
    taxonomy_codes: list[str] = Field(default_factory=list)
    addresses: list[dict[str, Any]] = Field(default_factory=list)
    source_snapshot_hash: str


class NormalizedExclusion(BaseModel):
    source_id: str
    source_record_id: str
    observed_at: datetime
    person_or_entity_name: str
    npi: str | None = None
    exclusion_date: datetime | None = None
    reinstatement_date: datetime | None = None
    exclusion_authority: str | None = None
    state: str | None = None
    source_snapshot_hash: str
