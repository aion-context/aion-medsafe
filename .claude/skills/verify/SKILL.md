---
description: Verify the integrity and authenticity of ingested data sources. Use when the user asks to verify provenance, check data integrity, or audit a manifest.
argument-hint: "[manifest_path]"
---

# Provenance Verification

## Steps

1. List all provenance manifests:
```bash
ls -la system/provenance/*.aion
```

2. Verify each manifest (or the specified one):
```bash
cd system && ./target/release/aion-medsafe provenance \
  --manifest $ARGUMENTS
```

3. For each manifest, confirm:
   - `Valid: true`
   - All four sub-checks pass (structure, integrity, hash chain, signatures)
   - Version count matches expected ingestion count

4. Cross-check data file hash if file path provided:
```bash
cd system && ./target/release/aion-medsafe verify \
  --manifest $0 \
  --file $1
```

5. Report any failures with full error context

## Tamper Test (Optional)
To prove tamper detection works:
```bash
cp $0 /tmp/tampered.aion
python3 -c "
data = bytearray(open('/tmp/tampered.aion','rb').read())
data[len(data)//2] ^= 0x01
open('/tmp/tampered.aion','wb').write(data)
"
cd system && ./target/release/aion-medsafe provenance --manifest /tmp/tampered.aion
# Should show: Valid: false, Integrity: false
```
