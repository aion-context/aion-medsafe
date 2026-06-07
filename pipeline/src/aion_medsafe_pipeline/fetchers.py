import hashlib
import pathlib
from datetime import UTC, datetime
from urllib.parse import urlparse
from urllib.request import Request, urlopen


def fetch_source(
    url: str,
    output_dir: pathlib.Path,
    *,
    user_agent: str = "aion-medsafe-pipeline/0.1",
    timeout: int = 120,
) -> tuple[pathlib.Path, str, datetime]:
    """Download a source asset, compute SHA-256, and save raw snapshot.

    Returns (saved_path, sha256_hex, fetched_at).
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    fetched_at = datetime.now(UTC)

    req = Request(url, headers={"User-Agent": user_agent})
    sha = hashlib.sha256()
    chunks: list[bytes] = []

    with urlopen(req, timeout=timeout) as resp:
        while True:
            chunk = resp.read(64 * 1024)
            if not chunk:
                break
            sha.update(chunk)
            chunks.append(chunk)

    digest = sha.hexdigest()
    parsed = urlparse(url)
    filename = pathlib.Path(parsed.path).name or "download"
    safe_name = f"{filename}"
    out_path = output_dir / safe_name

    with open(out_path, "wb") as f:
        for chunk in chunks:
            f.write(chunk)

    return out_path, digest, fetched_at
