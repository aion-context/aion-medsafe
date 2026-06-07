---
description: Run the full test suite including unit tests, property-based tests, and clippy lints. Use when the user asks to test, run tests, or check if things work.
---

# Run Test Suite

## Steps

### Rust System Tests (includes property-based tests)
```bash
cd system && cargo test 2>&1
```

### Rust Clippy (enforces Tiger Style)
```bash
cd system && cargo clippy -- -D warnings 2>&1
```

### Property-Based Tests (high iteration count)
```bash
cd system && PROPTEST_CASES=500 cargo test --test property_provenance 2>&1
```

### Python Pipeline Tests
```bash
cd pipeline && python -m pytest tests/ -v 2>&1
```

### Provenance Round-Trip Smoke Test
```bash
cd system
rm -rf /tmp/test_provenance && mkdir -p /tmp/test_provenance/.aion
echo "test data for provenance verification" > /tmp/test_provenance/test.csv
./target/release/aion-medsafe init --registry /tmp/test_provenance/.aion/medsafe.registry.json
./target/release/aion-medsafe ingest --file /tmp/test_provenance/test.csv --source test_source --output /tmp/test_provenance/test.aion
./target/release/aion-medsafe provenance --manifest /tmp/test_provenance/test.aion
```

## Success Criteria
- All Rust tests pass (including 7 property tests)
- Clippy reports zero warnings
- All Python tests pass
- Provenance round-trip: 4/4 guarantees pass
