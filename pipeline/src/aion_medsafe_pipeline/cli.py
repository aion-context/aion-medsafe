import json
import pathlib

import typer
from rich.console import Console
from rich.table import Table

from aion_medsafe_pipeline.fetchers import fetch_source
from aion_medsafe_pipeline.parsers import parse_leie_csv
from aion_medsafe_pipeline.sources import list_sources

app = typer.Typer(no_args_is_help=True, invoke_without_command=False)
console = Console()


@app.callback()
def main() -> None:
    pass


@app.command()
def sources(json_output: bool = typer.Option(False, "--json", help="Emit source registry as JSON.")) -> None:
    registry = list_sources()

    if json_output:
        console.print(json.dumps([source.model_dump(mode="json") for source in registry], indent=2))
        return

    table = Table(title="AION-MEDSAFE Public Data Sources")
    table.add_column("Source ID")
    table.add_column("Owner")
    table.add_column("Cadence")
    table.add_column("Priority")
    table.add_column("Access")

    for source in registry:
        table.add_row(
            source.source_id,
            source.owner,
            source.refresh_cadence.value,
            source.priority.value,
            source.access_method.value,
        )

    console.print(table)


@app.command()
def fetch(
    source: str = typer.Argument(..., help="Source ID to fetch (e.g., leie)"),
    output_dir: pathlib.Path = typer.Option(pathlib.Path("data"), "--output", "-o", help="Output directory for raw and normalized data"),
    max_records: int = typer.Option(10, "--max-records", "-n", help="Max normalized records to display"),
) -> None:
    """Fetch raw public data and parse into normalized records."""
    if source.lower() == "leie":
        url = "https://oig.hhs.gov/exclusions/downloadables/UPDATED.csv"
        source_id = "hhs_oig_leie"
        console.print(f"[bold]Fetching[/bold] {url}")
        raw_dir = output_dir / "raw"
        raw_path, digest, fetched_at = fetch_source(url, raw_dir)
        console.print(f"  Raw saved: {raw_path}")
        console.print(f"  SHA-256: {digest}")
        console.print(f"  Fetched at: {fetched_at.isoformat()}")

        console.print(f"\n[bold]Parsing[/bold] into normalized exclusions...")
        records = parse_leie_csv(raw_path, source_id=source_id, source_url=url, snapshot_hash=digest, fetched_at=fetched_at)
        console.print(f"  Parsed {len(records)} records")

        norm_dir = output_dir / "normalized"
        norm_dir.mkdir(parents=True, exist_ok=True)
        norm_path = norm_dir / "leie_normalized.ndjson"
        with open(norm_path, "w", encoding="utf-8") as f:
            for r in records:
                f.write(json.dumps(r.model_dump(mode="json")) + "\n")
        console.print(f"  Normalized saved: {norm_path}")

        console.print(f"\n[bold]First {min(max_records, len(records))} records:[/bold]")
        for r in records[:max_records]:
            console.print(f"  {r.person_or_entity_name} | NPI={r.npi} | Excl={r.exclusion_date} | Reinst={r.reinstatement_date} | State={r.state}")

    elif source.lower() == "leie-supplements":
        import re
        from html.parser import HTMLParser
        from urllib.parse import urljoin
        from urllib.request import Request as Req
        from urllib.request import urlopen as uopen

        class _LinkParser(HTMLParser):
            def __init__(self):
                super().__init__()
                self.links: list[str] = []
            def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
                if tag.lower() == "a":
                    href = dict(attrs).get("href")
                    if href:
                        self.links.append(href)

        index_url = "https://oig.hhs.gov/exclusions/leie-database-supplement-downloads/"
        console.print(f"[bold]Discovering supplements from[/bold] {index_url}")
        req = Req(index_url, headers={"User-Agent": "aion-medsafe-pipeline/0.1"})
        with uopen(req, timeout=30) as resp:
            html = resp.read().decode("utf-8", errors="replace")
        parser_html = _LinkParser()
        parser_html.feed(html)
        csv_urls = sorted(set(
            urljoin(index_url, href)
            for href in parser_html.links
            if re.search(r"(excl|rein)\.csv", href.lower())
        ))
        console.print(f"  Found {len(csv_urls)} supplement CSVs")

        raw_dir = output_dir / "raw" / "leie_supplements"
        all_records: list = []

        for csv_url in csv_urls:
            filename = csv_url.split("/")[-1]
            console.print(f"  Fetching {filename}...")
            raw_path, digest, fetched_at = fetch_source(csv_url, raw_dir)
            is_reinstatement = "rein" in filename.lower()
            records = parse_leie_csv(
                raw_path,
                source_id="hhs_oig_leie_supplement",
                source_url=csv_url,
                snapshot_hash=digest,
                fetched_at=fetched_at,
            )
            for r in records:
                all_records.append(r)

        console.print(f"\n[bold]Total supplement records:[/bold] {len(all_records)}")

        norm_dir = output_dir / "normalized"
        norm_dir.mkdir(parents=True, exist_ok=True)
        norm_path = norm_dir / "leie_supplements_normalized.ndjson"
        with open(norm_path, "w", encoding="utf-8") as f:
            for r in all_records:
                f.write(json.dumps(r.model_dump(mode="json")) + "\n")
        console.print(f"  Saved: {norm_path}")

        reinst_count = sum(1 for r in all_records if r.reinstatement_date)
        excl_count = len(all_records) - reinst_count
        npi_count = sum(1 for r in all_records if r.npi)
        console.print(f"  Exclusions: {excl_count}")
        console.print(f"  Reinstatements: {reinst_count}")
        console.print(f"  With NPI: {npi_count}")

        console.print(f"\n[bold]First {min(max_records, len(all_records))} records:[/bold]")
        for r in all_records[:max_records]:
            console.print(f"  {r.person_or_entity_name} | NPI={r.npi} | Excl={r.exclusion_date} | Reinst={r.reinstatement_date} | State={r.state}")

    elif source.lower() == "nppes":
        console.print("[bold]Enriching excluded providers with NPPES NPI data...[/bold]")

        leie_path = output_dir / "normalized" / "leie_normalized.ndjson"
        if not leie_path.exists():
            console.print("[red]Run 'fetch leie' first to generate the normalized LEIE file.[/red]")
            raise typer.Exit(code=1)

        npis: list[str] = []
        with open(leie_path, "r") as f:
            for line in f:
                rec = json.loads(line)
                if rec.get("npi"):
                    npis.append(rec["npi"])

        unique_npis = sorted(set(npis))
        console.print(f"  Unique NPIs to look up: {len(unique_npis)}")

        from urllib.request import Request as Req
        from urllib.request import urlopen as uopen
        import hashlib
        from datetime import UTC, datetime

        norm_dir = output_dir / "normalized"
        norm_dir.mkdir(parents=True, exist_ok=True)
        norm_path = norm_dir / "nppes_providers.ndjson"

        fetched_at = datetime.now(UTC)
        results = []
        errors = 0
        batch_size = min(len(unique_npis), max_records) if max_records else len(unique_npis)
        console.print(f"  Fetching first {batch_size} NPIs from NPPES API...")

        for i, npi in enumerate(unique_npis[:batch_size]):
            try:
                api_url = f"https://npiregistry.cms.hhs.gov/api/?version=2.1&number={npi}"
                req = Req(api_url, headers={"User-Agent": "aion-medsafe-pipeline/0.1"})
                with uopen(req, timeout=20) as resp:
                    raw = resp.read()
                data = json.loads(raw)
                if data.get("result_count", 0) > 0:
                    result = data["results"][0]
                    result["_source_id"] = "cms_nppes_npi_registry"
                    result["_fetched_at"] = fetched_at.isoformat()
                    result["_snapshot_hash"] = hashlib.sha256(raw).hexdigest()
                    results.append(result)
            except Exception:
                errors += 1
            if (i + 1) % 50 == 0:
                console.print(f"    ...{i + 1}/{batch_size} done")

        with open(norm_path, "w", encoding="utf-8") as f:
            for r in results:
                f.write(json.dumps(r) + "\n")

        console.print(f"\n[bold]NPPES fetch complete[/bold]")
        console.print(f"  Fetched: {len(results)}")
        console.print(f"  Errors: {errors}")
        console.print(f"  Saved: {norm_path}")

        if results:
            sample = results[0]
            console.print(f"\n[bold]Sample record keys:[/bold] {sorted(sample.keys())}")
            basic = sample.get("basic", {})
            console.print(f"[bold]Sample basic:[/bold] {json.dumps(basic, indent=2)}")
            console.print(f"[bold]Addresses:[/bold] {len(sample.get('addresses', []))}")
            console.print(f"[bold]Taxonomies:[/bold] {len(sample.get('taxonomies', []))}")

    else:
        console.print(f"[red]Unknown source: {source}[/red]")
        raise typer.Exit(code=1)


if __name__ == "__main__":
    app()
