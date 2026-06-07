# Source Retrieval Status

Validated on 2026-06-06 using lightweight HTTP probes. Large files were not fully downloaded.

## Summary

| Source | Status | Retrieval Notes | Next Implementation Step |
| --- | --- | --- | --- |
| NPPES Downloadable Files (bulk) | Implemented | `nppes_bulk.py` scrapes the index, downloads the full monthly dissemination + deactivation ZIPs, and normalizes the npidata CSV (field map ported from the npi-verify PoC) into the national NPI status table. **This is the bulk-first NPPES source** (ADR-001). | Run `nppes-bulk`; seal the raw ZIP via `ingest`. |
| NPPES NPI Registry API | Retired | Per-NPI API replaced by the bulk file (full coverage, no rate limits). | — (kept only as a historical note). |
| HHS-OIG LEIE | Reachable | Download page exposes `UPDATED.csv`, monthly exclusion supplements, reinstatement supplements, and record layout PDF. | Implement CSV downloader/parser for current database and monthly supplements. |
| Hawaii Med-QUEST Exclusion/Reinstatement List | Reachable | Public page exposes a current PDF exclusion/reinstatement list. Representative PDF returned `application/pdf` with `Last-Modified`. | Implement page parser and PDF extraction pipeline with manual review fallback. |
| SAM.gov Exclusions | Docs reachable | OpenAPI spec is reachable. API likely requires key-aware client behavior depending on endpoint and query. | Inspect OpenAPI spec, define auth/config handling, then implement API client. |
| CMS Public Provider Enrollment | Landing page reachable | Dataset page is reachable, but direct data/API asset URL was not discovered from static HTML probe. | Investigate CMS Data API/Socrata-style metadata or dataset export endpoint. |

## Confirmed Representative Assets

- `https://npiregistry.cms.hhs.gov/api/?version=2.1&number=1679576722`
- `https://download.cms.gov/nppes/NPPES_Data_Dissemination_052526_053126_Weekly_V2.zip`
- `https://oig.hhs.gov/exclusions/downloadables/UPDATED.csv`
- `https://oig.hhs.gov/exclusions/downloadables/2026/2604excl.csv`
- `https://medquest.hawaii.gov/content/dam/formsanddocuments/plans-and-providers/provider-exclusion-reinstatement-list/med-prov-excel-rein-list-2026/Med%20Prov%20Excl-Rein%20List-UPDATED-04.17.2026.pdf`
- `https://open.gsa.gov/api/exclusions-api/v1/openapi.yaml`

## Failure Modes To Handle

- Source page structure changes
- PDF layout changes
- Missing `Last-Modified` headers
- Large file download interruption
- CSV schema drift
- ZIP layout changes
- API rate limits or API key requirements
- Stale source pages that still return HTTP 200

## MVP Feasibility

The MVP public-data ingestion layer is feasible. The first fetchers should be implemented in this order:

1. NPPES NPI Registry targeted API client
2. HHS-OIG LEIE CSV downloader/parser
3. NPPES downloadable ZIP index parser
4. Hawaii Med-QUEST PDF downloader/extractor
5. SAM.gov exclusions API client
6. CMS provider enrollment dataset resolver
