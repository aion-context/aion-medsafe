import csv
import pathlib
from datetime import UTC, datetime

from aion_medsafe_pipeline.models import NormalizedExclusion


def parse_leie_csv(
    raw_path: pathlib.Path,
    source_id: str,
    source_url: str,
    snapshot_hash: str,
    fetched_at: datetime,
) -> list[NormalizedExclusion]:
    """Parse HHS-OIG LEIE UPDATED.csv into normalized exclusion records."""
    records: list[NormalizedExclusion] = []

    with open(raw_path, "r", encoding="utf-8-sig", newline="") as f:
        reader = csv.DictReader(f)
        for i, row in enumerate(reader):
            try:
                npi_raw = row.get("NPI", "").strip()
                npi = npi_raw if npi_raw and npi_raw != "0000000000" else None

                exclusion_date = None
                reinstatement_date = None
                try:
                    ed = row.get("EXCLDATE", "").strip()
                    if ed and ed != "00000000":
                        exclusion_date = datetime.strptime(ed, "%Y%m%d").replace(tzinfo=UTC)
                except ValueError:
                    pass

                try:
                    rd = row.get("REINDATE", "").strip()
                    if rd and rd != "00000000":
                        reinstatement_date = datetime.strptime(rd, "%Y%m%d").replace(tzinfo=UTC)
                except ValueError:
                    pass

                name_parts = []
                first = row.get("FIRSTNAME", "").strip()
                last = row.get("LASTNAME", "").strip()
                mid = row.get("MIDNAME", "").strip()
                bus = row.get("BUSNAME", "").strip()

                if bus:
                    name = bus
                else:
                    if last:
                        name_parts.append(last)
                    if first:
                        name_parts.append(first)
                    if mid:
                        name_parts.append(mid)
                    name = " ".join(name_parts) if name_parts else ""

                record = NormalizedExclusion(
                    source_id=source_id,
                    source_record_id=str(i),
                    observed_at=fetched_at,
                    person_or_entity_name=name,
                    npi=npi,
                    exclusion_date=exclusion_date,
                    reinstatement_date=reinstatement_date,
                    exclusion_authority="HHS-OIG",
                    state=row.get("STATE", "").strip() or None,
                    source_snapshot_hash=snapshot_hash,
                )
                records.append(record)
            except Exception:
                continue

    return records
