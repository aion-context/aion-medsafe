# Code Style

## Rust (system/)

### Formatting
- `rustfmt` with default settings
- Max line width: 100
- Imports grouped: std → external crates → internal modules
- One blank line between functions

### Naming
- Types: `PascalCase` — `ProviderEntity`, `RiskSignal`
- Functions: `snake_case` — `verify_file`, `seal_provenance`
- Constants: `SCREAMING_SNAKE` — `PIPELINE_AUTHOR`, `DEFAULT_KEY_PATH`
- Modules: `snake_case` — `provenance.rs`, `signals.rs`

### Function Size
- Target: under 40 lines
- Hard cap: 60 lines (Tiger Style)
- If a function exceeds 60 lines → extract helper functions

### Error Handling
```rust
// Return errors, don't log-and-continue
pub fn do_thing() -> Result<Output> {
    let data = load_data().map_err(|e| MedsafeError::ParseError {
        source_name: "thing".into(),
        reason: e.to_string(),
    })?;
    Ok(process(data))
}
```

### Tracing
```rust
// Structured events with bounded field names
tracing::info!(
    event = "provenance_sealed",      // fixed vocabulary
    source_id = source_id,            // what
    file = %path.display(),           // where
    size = file_size,                 // context
    manifest_hash = %hex::encode(id), // proof
);
```

## Python (pipeline/)

### Formatting
- Black with default settings (line length 88)
- isort for imports

### Naming
- Classes: `PascalCase` — `ProviderEntity`, `ExclusionEvent`
- Functions: `snake_case` — `parse_leie`, `normalize_record`
- Constants: `SCREAMING_SNAKE` — `LEIE_SOURCE_ID`
- Modules: `snake_case` — `parsers.py`, `lifecycle.py`

### Type Hints
- All public functions must have type annotations
- Use `dataclasses` for structured data
- Use `TypedDict` for JSON-like structures

### Logging
```python
import structlog
logger = structlog.get_logger()

logger.info("ingestion_complete",
    source=source_id,
    records_in=total,
    records_out=processed,
    errors=error_count,
)
```

### Docstrings
```python
def parse_leie(path: Path) -> list[ExclusionRecord]:
    """Parse LEIE UPDATED.csv into structured exclusion records.

    Args:
        path: Path to the raw LEIE CSV file.

    Returns:
        List of parsed exclusion records with normalized fields.

    Raises:
        ParseError: If the CSV format doesn't match expected schema.
    """
```

## Both Languages

### Comments
- Explain WHY, not WHAT
- No commented-out code in production
- TODO/FIXME are forbidden in Rust (Tiger Style), tracked as issues instead
- In Python, TODOs must reference an issue number: `# TODO(#42): implement fuzzy match`
