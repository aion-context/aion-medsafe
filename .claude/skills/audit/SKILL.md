---
description: Run a full system integrity audit covering registry health, provenance chains, data inventory, code quality, and security. Use when the user asks to audit, check system health, or run a full review.
---

# Full System Audit

## Steps

### 1. Registry Health
```bash
cat system/.aion/medsafe.registry.json | python3 -m json.tool
```
Verify: version 1, at least one author registered, all epochs have status.

### 2. Provenance Chain Integrity
```bash
cd system
for f in provenance/*.aion; do
  echo "=== $f ==="
  ./target/release/aion-medsafe provenance --manifest "$f" 2>&1 | grep -v "^{"
done
```
All manifests must show `Valid: true`.

### 3. Data Inventory
```bash
echo "=== Raw Sources ==="
find pipeline/data/raw -type f | wc -l
du -sh pipeline/data/raw/

echo "=== Normalized Outputs ==="
for f in pipeline/data/normalized/*.ndjson; do
  echo "  $(wc -l < "$f") records | $(basename $f)"
done
```

### 4. Code Quality
```bash
cd system && cargo clippy -- -D warnings 2>&1 | tail -3
cd ../pipeline && python -m py_compile src/aion_medsafe_pipeline/*.py && echo "✓ Python compiles"
```

### 5. Security Check
```bash
grep -r "PRIVATE\|SECRET\|password\|api_key" --include="*.py" --include="*.rs" --include="*.toml" . | grep -v target | grep -v .venv || echo "✓ No secrets found"
```

### 6. Property Tests
```bash
cd system && cargo test --test property_provenance
```

## Report Format
```
AION-MEDSAFE Audit Report
Date: YYYY-MM-DD
────────────────────────
Registry:     [OK/FAIL]
Provenance:   [OK/FAIL] (N manifests verified)
Data:         [OK/FAIL] (N sources, M total records)
Code Quality: [OK/FAIL]
Security:     [OK/FAIL]
Tests:        [OK/FAIL]
────────────────────────
Overall:      [PASS/FAIL]
```
