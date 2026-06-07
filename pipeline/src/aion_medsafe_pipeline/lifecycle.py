"""Lifecycle Event Store for temporal exclusion/reinstatement tracking.

De-risks the problem of supplements not being in the main LEIE file and
reinstatement events being invisible.

Techniques:
- Event sourcing: every exclusion and reinstatement is an immutable event
- Entity timeline reconstruction: merge events across sources into a single timeline
- Temporal queries: "was this provider excluded on date X?"
- Gap detection: find providers excluded then reinstated then re-excluded
- Source attribution: every event is traceable to its source record

This enables the Rust system to answer:
- "Was this provider excluded when this claim was submitted?"
- "Has this provider been excluded more than once?"
- "Was there billing activity between exclusion and reinstatement?"
"""

from __future__ import annotations

import json
import pathlib
from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import StrEnum
from typing import Any


class EventType(StrEnum):
    EXCLUSION = "exclusion"
    REINSTATEMENT = "reinstatement"


@dataclass(frozen=True)
class LifecycleEvent:
    entity_key: str  # normalized name or NPI
    event_type: EventType
    event_date: datetime | None
    source_id: str
    source_record_id: str
    source_snapshot_hash: str
    observed_at: datetime
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass
class EntityTimeline:
    entity_key: str
    events: list[LifecycleEvent] = field(default_factory=list)

    @property
    def is_currently_excluded(self) -> bool:
        """True if the most recent event is an exclusion (not reinstated)."""
        if not self.events:
            return False
        sorted_events = sorted(
            self.events,
            key=lambda e: e.event_date or datetime.min.replace(tzinfo=UTC),
        )
        return sorted_events[-1].event_type == EventType.EXCLUSION

    @property
    def exclusion_count(self) -> int:
        return sum(1 for e in self.events if e.event_type == EventType.EXCLUSION)

    @property
    def reinstatement_count(self) -> int:
        return sum(1 for e in self.events if e.event_type == EventType.REINSTATEMENT)

    @property
    def has_re_exclusion(self) -> bool:
        """True if provider was excluded, reinstated, then excluded again."""
        return self.exclusion_count > 1 and self.reinstatement_count >= 1

    def was_excluded_on(self, date: datetime) -> bool:
        """Determine if entity was excluded on a specific date."""
        sorted_events = sorted(
            self.events,
            key=lambda e: e.event_date or datetime.min.replace(tzinfo=UTC),
        )
        status = False
        for event in sorted_events:
            if event.event_date and event.event_date <= date:
                status = event.event_type == EventType.EXCLUSION
            else:
                break
        return status

    def exclusion_windows(self) -> list[tuple[datetime | None, datetime | None]]:
        """Return list of (exclusion_start, exclusion_end) windows."""
        sorted_events = sorted(
            self.events,
            key=lambda e: e.event_date or datetime.min.replace(tzinfo=UTC),
        )
        windows: list[tuple[datetime | None, datetime | None]] = []
        current_start: datetime | None = None

        for event in sorted_events:
            if event.event_type == EventType.EXCLUSION:
                current_start = event.event_date
            elif event.event_type == EventType.REINSTATEMENT:
                windows.append((current_start, event.event_date))
                current_start = None

        # If still excluded (no reinstatement closing)
        if current_start is not None:
            windows.append((current_start, None))

        return windows


class LifecycleStore:
    """In-memory event store for entity lifecycle tracking."""

    def __init__(self) -> None:
        self._events: list[LifecycleEvent] = []
        self._timelines: dict[str, EntityTimeline] = {}

    @property
    def event_count(self) -> int:
        return len(self._events)

    @property
    def entity_count(self) -> int:
        return len(self._timelines)

    def add_event(self, event: LifecycleEvent) -> None:
        self._events.append(event)
        if event.entity_key not in self._timelines:
            self._timelines[event.entity_key] = EntityTimeline(entity_key=event.entity_key)
        self._timelines[event.entity_key].events.append(event)

    def get_timeline(self, entity_key: str) -> EntityTimeline | None:
        return self._timelines.get(entity_key)

    def currently_excluded(self) -> list[EntityTimeline]:
        return [t for t in self._timelines.values() if t.is_currently_excluded]

    def re_excluded_entities(self) -> list[EntityTimeline]:
        """Entities with multiple exclusions — high-risk pattern."""
        return [t for t in self._timelines.values() if t.has_re_exclusion]

    def save(self, path: pathlib.Path) -> None:
        """Persist event store as NDJSON."""
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            for event in self._events:
                record = {
                    "entity_key": event.entity_key,
                    "event_type": event.event_type.value,
                    "event_date": event.event_date.isoformat() if event.event_date else None,
                    "source_id": event.source_id,
                    "source_record_id": event.source_record_id,
                    "source_snapshot_hash": event.source_snapshot_hash,
                    "observed_at": event.observed_at.isoformat(),
                    "metadata": event.metadata,
                }
                f.write(json.dumps(record) + "\n")

    def load(self, path: pathlib.Path) -> None:
        """Load event store from NDJSON."""
        if not path.exists():
            return
        with open(path, "r", encoding="utf-8") as f:
            for line in f:
                record = json.loads(line)
                event_date = None
                if record["event_date"]:
                    event_date = datetime.fromisoformat(record["event_date"])
                event = LifecycleEvent(
                    entity_key=record["entity_key"],
                    event_type=EventType(record["event_type"]),
                    event_date=event_date,
                    source_id=record["source_id"],
                    source_record_id=record["source_record_id"],
                    source_snapshot_hash=record["source_snapshot_hash"],
                    observed_at=datetime.fromisoformat(record["observed_at"]),
                    metadata=record.get("metadata", {}),
                )
                self.add_event(event)


def build_lifecycle_from_leie(normalized_path: pathlib.Path, supplements_path: pathlib.Path | None = None) -> LifecycleStore:
    """Build a lifecycle store from LEIE normalized data."""
    from aion_medsafe_pipeline.entity_resolution import normalize_name

    store = LifecycleStore()

    # Process main LEIE file
    if normalized_path.exists():
        with open(normalized_path, "r") as f:
            for line in f:
                record = json.loads(line)
                name = record.get("person_or_entity_name", "")
                npi = record.get("npi")
                entity_key = npi if npi else normalize_name(name)

                if record.get("exclusion_date"):
                    excl_date = datetime.fromisoformat(record["exclusion_date"])
                    store.add_event(LifecycleEvent(
                        entity_key=entity_key,
                        event_type=EventType.EXCLUSION,
                        event_date=excl_date,
                        source_id=record.get("source_id", "unknown"),
                        source_record_id=record.get("source_record_id", ""),
                        source_snapshot_hash=record.get("source_snapshot_hash", ""),
                        observed_at=datetime.fromisoformat(record["observed_at"]),
                        metadata={"name": name, "npi": npi, "state": record.get("state")},
                    ))

                if record.get("reinstatement_date"):
                    reinst_date = datetime.fromisoformat(record["reinstatement_date"])
                    store.add_event(LifecycleEvent(
                        entity_key=entity_key,
                        event_type=EventType.REINSTATEMENT,
                        event_date=reinst_date,
                        source_id=record.get("source_id", "unknown"),
                        source_record_id=record.get("source_record_id", ""),
                        source_snapshot_hash=record.get("source_snapshot_hash", ""),
                        observed_at=datetime.fromisoformat(record["observed_at"]),
                        metadata={"name": name, "npi": npi, "state": record.get("state")},
                    ))

    # Process supplements (these contain reinstatement data)
    if supplements_path and supplements_path.exists():
        with open(supplements_path, "r") as f:
            for line in f:
                record = json.loads(line)
                name = record.get("person_or_entity_name", "")
                npi = record.get("npi")
                entity_key = npi if npi else normalize_name(name)

                if record.get("exclusion_date"):
                    excl_date = datetime.fromisoformat(record["exclusion_date"])
                    store.add_event(LifecycleEvent(
                        entity_key=entity_key,
                        event_type=EventType.EXCLUSION,
                        event_date=excl_date,
                        source_id=record.get("source_id", "unknown"),
                        source_record_id=record.get("source_record_id", ""),
                        source_snapshot_hash=record.get("source_snapshot_hash", ""),
                        observed_at=datetime.fromisoformat(record["observed_at"]),
                        metadata={"name": name, "npi": npi, "state": record.get("state")},
                    ))

                if record.get("reinstatement_date"):
                    reinst_date = datetime.fromisoformat(record["reinstatement_date"])
                    store.add_event(LifecycleEvent(
                        entity_key=entity_key,
                        event_type=EventType.REINSTATEMENT,
                        event_date=reinst_date,
                        source_id=record.get("source_id", "unknown"),
                        source_record_id=record.get("source_record_id", ""),
                        source_snapshot_hash=record.get("source_snapshot_hash", ""),
                        observed_at=datetime.fromisoformat(record["observed_at"]),
                        metadata={"name": name, "npi": npi, "state": record.get("state")},
                    ))

    return store
