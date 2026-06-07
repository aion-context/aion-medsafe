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

    else:
        console.print(f"[red]Unknown source: {source}[/red]")
        raise typer.Exit(code=1)


@app.command(name="nppes-bulk")
def nppes_bulk_command(
    data_dir: pathlib.Path = typer.Option(
        pathlib.Path("data"), "--data", "-d", help="Pipeline data directory."
    ),
    monthly_url: str = typer.Option(None, "--monthly-url", help="Override monthly ZIP URL."),
    limit: int = typer.Option(
        None, "--limit", "-l", help="Cap rows written (for a quick partial run)."
    ),
) -> None:
    """Download the full CMS NPPES bulk file and normalize it to a national NPI
    status table (NDJSON). This is the bulk-first replacement for per-NPI API
    lookups; it powers `active_npi_while_excluded` with 100% coverage.

    Seal the raw ZIP afterwards with `aion-medsafe ingest` for provenance.
    """
    from aion_medsafe_pipeline.nppes_bulk import download_bulk, process_bulk_zip

    console.print("[bold]Downloading NPPES bulk dissemination file...[/bold]")
    hashes = download_bulk(data_dir, monthly_url=monthly_url)
    monthly_path = next(
        pathlib.Path(p) for p in hashes if "Dissemination" in p and "Weekly" not in p
    )
    snapshot_hash = hashes[str(monthly_path)]
    console.print(f"  Monthly ZIP: {monthly_path.name}")
    console.print(f"  SHA-256: {snapshot_hash}")

    # Stream the CSV out of the ZIP (never extracts the ~9 GB uncompressed file).
    console.print("[bold]Normalizing (streaming from ZIP)...[/bold]")
    out_path = data_dir / "normalized" / "nppes_providers.ndjson"
    stats = process_bulk_zip(monthly_path, out_path, snapshot_hash, limit=limit)
    for key, value in stats.items():
        console.print(f"  {key}: {value:,}")
    console.print(f"  Saved: {out_path}")
    console.print(
        f"\n[dim]Next: aion-medsafe ingest --file {monthly_path} --source nppes_bulk "
        "(seal raw source), then aion-medsafe build-graph[/dim]"
    )


if __name__ == "__main__":
    app()
