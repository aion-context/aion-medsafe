# Testing Discipline

## Scope
All code changes across both pipeline and system.

## Principle: Tests Are Proof

In a system that generates evidence for fraud investigations, "it works on my machine" is not acceptable. Tests ARE the proof that the system behaves correctly.

## Test Hierarchy

| Level | What | Where | Runs When |
|---|---|---|---|
| Unit | Individual functions | `system/tests/`, `pipeline/tests/` | Every change |
| Integration | Cross-module paths | `tests/integration/` | Every PR |
| Provenance | aion-context seal/verify round-trip | `system/tests/` | Every change to provenance |
| Tamper | Flip bytes, verify detection | `system/tests/` | Every change to verification |
| Cross-reference | Multi-source entity matching | `pipeline/tests/` | Data schema changes |
| End-to-end | Full ingest → signal pipeline | `tests/e2e/` | Release |

## Required Test Cases for Every Signal Type

For each risk signal definition in the policy:
1. **True positive** — signal fires correctly on known-bad data
2. **True negative** — signal does NOT fire on known-good data
3. **Threshold boundary** — confidence exactly at threshold (both sides)
4. **Missing data** — graceful handling when a source field is absent
5. **Tampered policy** — system refuses to compute signal

## Rust Test Patterns

```rust
#[test]
fn test_provenance_seal_and_verify_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    // ... seal, then verify, assert all 4 guarantees pass
}

#[test]
fn test_tampered_file_detected() {
    // ... seal, flip byte, verify, assert is_valid == false
}

#[test]
fn test_policy_gate_refuses_tampered_policy() {
    // ... create policy, tamper it, attempt signal computation
    // ... assert error is PolicyIntegrityFailed
}
```

## Python Test Patterns

```python
def test_leie_normalization_preserves_record_count():
    """Raw CSV line count must equal normalized NDJSON line count."""
    raw_count = count_lines("data/raw/UPDATED.csv") - 1  # minus header
    normalized_count = count_lines("data/normalized/leie_normalized.ndjson")
    assert raw_count == normalized_count

def test_cross_reference_finds_known_overlap():
    """Known-excluded provider with deactivated NPI must appear in signals."""
    # Use synthetic test fixtures, not real PII
    ...
```

## What to Test BEFORE Implementation

When adding a new feature:
1. Write the test that would prove it works
2. Run it — confirm it fails (red)
3. Implement the minimum code to make it pass (green)
4. Refactor if needed (refactor)

## Forbidden in Tests

- No `sleep()` — use deterministic time
- No network calls — mock or use local fixtures
- No real PII — use synthetic providers
- No flaky tests — if it fails intermittently, fix the race condition

## Critical Gates (see adversarial-testing.md)

Property-based testing and mutation testing are NON-NEGOTIABLE gates:
- **Property tests** prove invariants hold for all inputs
- **Mutation tests** prove the tests actually catch bugs

Code does not merge without:
1. All property tests passing (100%)
2. Mutation score ≥ 85% overall
3. Mutation score ≥ 95% on critical paths (provenance, policy, parsing)

See `.claude/rules/adversarial-testing.md` for full protocol.
