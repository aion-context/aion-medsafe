---
description: Build release artifacts, verify all provenance, and package for deployment. Use when the user asks to build, deploy, or release.
---

# Build and Deploy

## Steps

### 1. Full Test Suite (gate)
```bash
cd system && cargo test && cargo clippy -- -D warnings
```
If tests fail — STOP. Do not deploy broken code.

### 2. Build Release Binary
```bash
cd system && cargo build --release
ls -lh target/release/aion-medsafe
```

### 3. Verify Binary Works
```bash
cd system && ./target/release/aion-medsafe --version
./target/release/aion-medsafe --help
```

### 4. Verify All Provenance
```bash
cd system
for f in provenance/*.aion; do
  ./target/release/aion-medsafe provenance --manifest "$f" 2>&1 | grep "Valid:" | grep -q "true" || { echo "FAIL: $f"; exit 1; }
done
echo "✓ All provenance verified"
```

### 5. Package
```bash
mkdir -p release/
cp system/target/release/aion-medsafe release/
cp system/.aion/medsafe.registry.json release/
cp -r system/provenance/ release/provenance/
echo "AION-MEDSAFE Release $(date +%Y-%m-%d)" > release/MANIFEST.txt
echo "Binary: $(sha256sum release/aion-medsafe)" >> release/MANIFEST.txt
```
