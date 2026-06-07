---
paths:
  - "system/src/**/*.rs"
  - "system/tests/**/*.rs"
---

# aion-context Integration Patterns

## Scope
All code that touches `.aion` files, the key registry, or provenance operations.

## Library Usage

### Correct Import Pattern
```rust
use aion_context::crypto::SigningKey;
use aion_context::key_registry::KeyRegistry;
use aion_context::manifest::{sign_manifest, ArtifactManifestBuilder};
use aion_context::operations::{init_file, commit_version, verify_file, show_current_rules, InitOptions, CommitOptions};
use aion_context::types::AuthorId;
```

### Registry Serialization
```rust
// Save registry (uses aion-context's own format)
let json = registry.to_trusted_json()?;
std::fs::write(path, &json)?;

// Load registry
let json = std::fs::read_to_string(path)?;
let registry = KeyRegistry::from_trusted_json(&json)?;
```

### Key Persistence
```rust
// Save (32 bytes, Ed25519 seed)
std::fs::write(key_path, key.to_bytes())?;

// Load
let bytes = std::fs::read(key_path)?;
let key = SigningKey::from_bytes(&bytes)?;
```

### Provenance Sealing
```rust
// Build manifest for external artifact
let mut builder = ArtifactManifestBuilder::new();
let _handle = builder.add("artifact_name", &raw_bytes);
let manifest = builder.build();
let manifest_id = *manifest.manifest_id(); // [u8; 32]

// Sign manifest
let sig = sign_manifest(&manifest, author_id, &signing_key);

// Create .aion file wrapping metadata as rules payload
let options = InitOptions {
    author_id,
    signing_key: &key,
    message: "commit message",
    timestamp: None, // uses current time
};
init_file(path, payload_bytes, &options)?;
```

### Verification (The Policy Loop Pattern)
```rust
// EVERY TIME you need to act on policy/data:
let report = verify_file(path, &registry)?;
if !report.is_valid {
    // REFUSE to act — structured rejection
    return Err(MedsafeError::PolicyIntegrityFailed {
        reasons: report.errors.clone(),
    });
}
// Only now extract and use the rules
let rules = show_current_rules(path)?;
```

### Version Commits (Appending to Existing Chain)
```rust
let options = CommitOptions {
    author_id,
    signing_key: &key,
    message: "Monthly refresh: June 2026",
    timestamp: None,
};
let result = commit_version(path, new_payload_bytes, &options)?;
// result.version is now N+1, parent_hash chains to version N
```

## Anti-Patterns (NEVER DO)

```rust
// ❌ NEVER: Skip verification
let rules = show_current_rules(path)?; // no verify_file first!

// ❌ NEVER: Cache verification results
static LAST_VERIFIED: OnceCell<bool> = OnceCell::new();

// ❌ NEVER: Use raw file I/O for .aion files
let raw = std::fs::read("policy.aion")?;
let rules = &raw[some_offset..]; // manual parsing

// ❌ NEVER: Log key material
tracing::info!(signing_key = ?key); // FORBIDDEN

// ❌ NEVER: Generate keys per-operation
let key = SigningKey::generate(); // use persistent key from init
```

## VerificationReport Fields
```rust
pub struct VerificationReport {
    pub file_id: FileId,
    pub version_count: u64,
    pub structure_valid: bool,      // header parses, sections in bounds
    pub integrity_hash_valid: bool, // trailing BLAKE3 matches contents
    pub hash_chain_valid: bool,     // every parent_hash link is consistent
    pub signatures_valid: bool,     // every signature verifies against registry
    pub is_valid: bool,             // all four must be true
    pub errors: Vec<String>,        // structured error messages
    pub temporal_warnings: Vec<TemporalWarning>,
}
```
