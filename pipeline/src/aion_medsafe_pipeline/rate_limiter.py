"""Rate-Limited Fetcher with exponential backoff, resume capability, and batch scheduling.

De-risks the NPPES API rate limit problem and general source availability issues.

Techniques:
- Token bucket rate limiting (configurable requests/second)
- Exponential backoff with jitter on transient failures
- Resume from checkpoint (skip already-fetched NPIs)
- Batch scheduling with progress tracking
- Circuit breaker pattern (stop after N consecutive failures)
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import time
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


@dataclass
class FetchResult:
    url: str
    success: bool
    status_code: int | None = None
    data: bytes | None = None
    sha256: str | None = None
    error: str | None = None
    elapsed_ms: float = 0.0
    fetched_at: datetime = field(default_factory=lambda: datetime.now(UTC))


@dataclass
class BatchProgress:
    total: int
    completed: int = 0
    successes: int = 0
    failures: int = 0
    skipped: int = 0
    consecutive_failures: int = 0


class RateLimitedFetcher:
    """Fetcher with rate limiting, backoff, resume, and circuit breaker."""

    def __init__(
        self,
        requests_per_second: float = 2.0,
        max_retries: int = 3,
        backoff_base: float = 1.0,
        backoff_max: float = 30.0,
        circuit_breaker_threshold: int = 10,
        user_agent: str = "aion-medsafe-pipeline/0.1",
        timeout: int = 20,
    ):
        self._interval = 1.0 / requests_per_second
        self._max_retries = max_retries
        self._backoff_base = backoff_base
        self._backoff_max = backoff_max
        self._circuit_breaker_threshold = circuit_breaker_threshold
        self._user_agent = user_agent
        self._timeout = timeout
        self._last_request_time: float = 0.0

    def _wait_for_slot(self) -> None:
        """Token bucket: wait until we can make the next request."""
        now = time.monotonic()
        elapsed = now - self._last_request_time
        if elapsed < self._interval:
            time.sleep(self._interval - elapsed)
        self._last_request_time = time.monotonic()

    def _backoff_sleep(self, attempt: int) -> None:
        """Exponential backoff with jitter."""
        import random
        delay = min(self._backoff_base * (2 ** attempt), self._backoff_max)
        jitter = random.uniform(0, delay * 0.3)
        time.sleep(delay + jitter)

    def fetch_one(self, url: str) -> FetchResult:
        """Fetch a single URL with retries and backoff."""
        for attempt in range(self._max_retries + 1):
            self._wait_for_slot()
            start = time.monotonic()
            try:
                req = Request(url, headers={"User-Agent": self._user_agent})
                with urlopen(req, timeout=self._timeout) as resp:
                    data = resp.read()
                    elapsed = (time.monotonic() - start) * 1000
                    digest = hashlib.sha256(data).hexdigest()
                    return FetchResult(
                        url=url,
                        success=True,
                        status_code=resp.status,
                        data=data,
                        sha256=digest,
                        elapsed_ms=elapsed,
                    )
            except HTTPError as e:
                elapsed = (time.monotonic() - start) * 1000
                if e.code == 429 or e.code >= 500:
                    if attempt < self._max_retries:
                        self._backoff_sleep(attempt)
                        continue
                return FetchResult(
                    url=url,
                    success=False,
                    status_code=e.code,
                    error=f"HTTP {e.code}: {e.reason}",
                    elapsed_ms=elapsed,
                )
            except (URLError, TimeoutError, OSError) as e:
                elapsed = (time.monotonic() - start) * 1000
                if attempt < self._max_retries:
                    self._backoff_sleep(attempt)
                    continue
                return FetchResult(
                    url=url,
                    success=False,
                    error=f"{type(e).__name__}: {e}",
                    elapsed_ms=elapsed,
                )

        return FetchResult(url=url, success=False, error="max retries exhausted")

    def fetch_batch(
        self,
        urls: list[str],
        checkpoint_path: pathlib.Path | None = None,
        on_progress: Any = None,
    ) -> tuple[list[FetchResult], BatchProgress]:
        """Fetch a batch of URLs with resume from checkpoint.

        checkpoint_path: NDJSON file of previously completed URLs (for resume).
        on_progress: callable(BatchProgress) called after each fetch.
        """
        # Load checkpoint (already-fetched URLs)
        completed_urls: set[str] = set()
        if checkpoint_path and checkpoint_path.exists():
            with open(checkpoint_path, "r") as f:
                for line in f:
                    try:
                        entry = json.loads(line)
                        completed_urls.add(entry["url"])
                    except (json.JSONDecodeError, KeyError):
                        continue

        progress = BatchProgress(total=len(urls))
        results: list[FetchResult] = []

        for url in urls:
            # Resume: skip already-fetched
            if url in completed_urls:
                progress.skipped += 1
                progress.completed += 1
                if on_progress:
                    on_progress(progress)
                continue

            # Circuit breaker
            if progress.consecutive_failures >= self._circuit_breaker_threshold:
                remaining = progress.total - progress.completed
                for _ in range(remaining):
                    results.append(FetchResult(
                        url=url,
                        success=False,
                        error="circuit_breaker_open",
                    ))
                    progress.failures += 1
                    progress.completed += 1
                break

            result = self.fetch_one(url)
            results.append(result)
            progress.completed += 1

            if result.success:
                progress.successes += 1
                progress.consecutive_failures = 0
                # Write to checkpoint
                if checkpoint_path:
                    checkpoint_path.parent.mkdir(parents=True, exist_ok=True)
                    with open(checkpoint_path, "a") as f:
                        f.write(json.dumps({
                            "url": url,
                            "sha256": result.sha256,
                            "fetched_at": result.fetched_at.isoformat(),
                        }) + "\n")
            else:
                progress.failures += 1
                progress.consecutive_failures += 1

            if on_progress:
                on_progress(progress)

        return results, progress
