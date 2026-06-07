# Adversarial Testing — Property-Based & Mutation Testing

## Principle

Unit tests prove "it works for these examples."
Property-based tests prove "it works for ALL inputs satisfying these invariants."
Mutation tests prove "our tests actually catch bugs, not just execute code."

Both are **critical gates**. Code does not merge without passing both.

## Property-Based Testing (PBT)

### Rust: `proptest`
```toml
# system/Cargo.toml [dev-dependencies]
proptest = "1"
```

### Python: `hypothesis`
```toml
# pipeline/pyproject.toml [tool.pytest]
hypothesis = ">=6.0"
```

### What Properties Look Like

Properties are universal truths about your code. They hold for ALL valid inputs, not just hand-picked examples.

#### Round-Trip Properties
```rust
// If I seal provenance and verify it, it ALWAYS passes
proptest! {
    #[test]
    fn provenance_seal_verify_roundtrip(
        payload in prop::collection::vec(any::<u8>(), 1..10_000),
        source_id in "[a-z_]{3,20}",
    ) {
        let dir = tempfile::tempdir()?;
        let key = SigningKey::generate();
        let mut registry = KeyRegistry::new();
        let author = AuthorId::new(80001);
        registry.register_author(author, key.verifying_key(), key.verifying_key(), 0)?;

        let path = dir.path().join("test.aion");
        init_file(&path, &payload, &InitOptions {
            author_id: author, signing_key: &key,
            message: "test", timestamp: None,
        })?;

        let report = verify_file(&path, &registry)?;
        prop_assert!(report.is_valid);
        prop_assert!(report.structure_valid);
        prop_assert!(report.integrity_hash_valid);
        prop_assert!(report.hash_chain_valid);
        prop_assert!(report.signatures_valid);
    }
}
```

#### Tamper-Detection Property
```rust
// If I flip ANY single byte in a sealed file, verification ALWAYS fails
proptest! {
    #[test]
    fn single_byte_flip_always_detected(
        payload in prop::collection::vec(any::<u8>(), 100..5_000),
        flip_offset_pct in 0.0f64..1.0,
    ) {
        // seal the file
        let (path, registry) = seal_test_file(&payload)?;

        // flip one byte at a random position
        let mut bytes = std::fs::read(&path)?;
        let offset = (flip_offset_pct * (bytes.len() - 1) as f64) as usize;
        bytes[offset] ^= 0x01;
        std::fs::write(&path, &bytes)?;

        // verification MUST fail
        let report = verify_file(&path, &registry)?;
        prop_assert!(!report.is_valid);
    }
}
```

#### Normalization Idempotency
```python
from hypothesis import given, strategies as st

@given(name=st.text(min_size=1, max_size=100))
def test_name_normalization_is_idempotent(name):
    """Normalizing twice produces same result as normalizing once."""
    once = normalize_name(name)
    twice = normalize_name(once)
    assert once == twice

@given(npi=st.from_regex(r'[0-9]{10}', fullmatch=True))
def test_npi_validation_accepts_all_10_digit_strings(npi):
    """Any 10-digit string is a structurally valid NPI."""
    assert is_valid_npi_format(npi)

@given(npi=st.text().filter(lambda s: not s.isdigit() or len(s) != 10))
def test_npi_validation_rejects_invalid_format(npi):
    """Anything that isn't exactly 10 digits is rejected."""
    assert not is_valid_npi_format(npi)
```

#### Entity Resolution Properties
```python
@given(
    first=st.text(min_size=1, max_size=50),
    last=st.text(min_size=1, max_size=50),
)
def test_canonical_id_is_deterministic(first, last):
    """Same input always produces same canonical ID."""
    id1 = compute_canonical_id(first, last, "HI", "1234567890")
    id2 = compute_canonical_id(first, last, "HI", "1234567890")
    assert id1 == id2

@given(data=st.data())
def test_canonical_id_collision_resistance(data):
    """Different providers produce different IDs (with overwhelming probability)."""
    first1 = data.draw(st.text(min_size=1, max_size=50))
    first2 = data.draw(st.text(min_size=1, max_size=50).filter(lambda x: x != first1))
    id1 = compute_canonical_id(first1, "SMITH", "HI", "1234567890")
    id2 = compute_canonical_id(first2, "SMITH", "HI", "1234567890")
    assert id1 != id2
```

### Core Properties for AION-MEDSAFE

| Module | Property | Invariant |
|---|---|---|
| Provenance | Seal/verify round-trip | `seal → verify = VALID` for all payloads |
| Provenance | Tamper detection | `seal → flip any byte → verify = INVALID` |
| Provenance | Hash determinism | Same bytes → same BLAKE3 hash, always |
| Chain | Append preserves history | `commit(v2) → verify(v1 chain) still valid` |
| Chain | Order matters | Reordering versions breaks `parent_hash` |
| Policy | Parse/serialize round-trip | `parse(serialize(policy)) == policy` |
| Normalization | Idempotency | `normalize(normalize(x)) == normalize(x)` |
| Entity resolution | Determinism | Same inputs → same canonical ID |
| Entity resolution | Symmetry | `match(a, b) == match(b, a)` |
| Risk signals | Monotonicity | More evidence → confidence never decreases |
| Risk signals | Threshold boundary | Confidence at exactly threshold → correct classification |

### PBT Failure = Bug in Code OR Bug in Property

When a property test fails:
1. Read the shrunk counterexample carefully
2. Determine: is the CODE wrong, or is the PROPERTY wrong?
3. If code is wrong → fix code, property stays
4. If property is wrong → tighten the preconditions, document why

Never weaken a property to make a test pass. Tighten preconditions instead.

---

## Mutation Testing

### Rust: `cargo-mutants`
```bash
cargo install cargo-mutants
cd system && cargo mutants --timeout 60
```

### Python: `mutmut`
```bash
pip install mutmut
cd pipeline && mutmut run --paths-to-mutate src/aion_medsafe_pipeline/
```

### What Mutation Testing Proves

A mutation test:
1. Makes a small change to your source code (a "mutant") — e.g., `>` → `>=`, `+` → `-`, `true` → `false`
2. Runs your test suite against the mutant
3. If tests PASS with the mutant alive → **your tests are weak** (they don't actually verify that line)
4. If tests FAIL → mutant is "killed" → your tests are effective

### Mutation Score Target

| Score | Assessment |
|---|---|
| < 70% | Tests are theater. Code is undertested. |
| 70–85% | Acceptable for non-critical code |
| 85–95% | Good. Expected for most modules. |
| > 95% | Excellent. Required for security-critical paths. |

### Critical Paths (95%+ mutation score required)

- `system/src/provenance.rs` — chain of custody
- `system/src/policy.rs` — policy verification gate
- `system/src/error.rs` — error classification
- `pipeline/src/aion_medsafe_pipeline/parsers.py` — data parsing correctness
- `pipeline/src/aion_medsafe_pipeline/entity_resolution.py` — matching logic

### Surviving Mutants = Missing Tests

When a mutant survives:
1. Read the mutation: what was changed?
2. Ask: "Should my tests have caught this?"
3. If yes → write the missing test case
4. If no (e.g., cosmetic change) → mark as equivalent mutant

### Example Mutations That MUST Be Killed

```rust
// Original
if !report.is_valid {
    return Err(MedsafeError::PolicyIntegrityFailed { ... });
}

// Mutant: remove the negation
if report.is_valid {  // ← this mutant MUST be killed
    return Err(MedsafeError::PolicyIntegrityFailed { ... });
}
// If this survives, your test never exercises the tampered-policy path
```

```python
# Original
if confidence >= threshold:
    queue_for_review(signal)

# Mutant: change >= to >
if confidence > threshold:  # ← must be killed
    queue_for_review(signal)
# If this survives, you have no boundary test at exactly threshold
```

---

## Gate Configuration

### Pre-Commit (Fast)
- Property tests with reduced iterations (`PROPTEST_CASES=50`)
- Mutation testing SKIPPED (too slow for commit hook)

### Pre-Push (Thorough)
- Property tests with full iterations (`PROPTEST_CASES=1000`)
- Mutation testing on changed files only:
```bash
changed=$(git diff --name-only origin/main | grep '\.rs$')
cargo mutants --timeout 60 -- $changed
```

### CI (Exhaustive)
- Property tests: `PROPTEST_CASES=10000`
- Full mutation testing: all critical paths
- Mutation score gate: fail CI if score < 85% overall, < 95% on critical paths

---

## Integration with aion-context's Own Testing Philosophy

aion-context itself uses:
- **Hegel property tests** — `prop_parser_new_never_panics_on_arbitrary_bytes`
- **libFuzzer** — 27.4M iterations on parser, zero panics
- **Tiger Style** — zero tolerance for any panic path

Our tests extend this philosophy to the application layer:
- aion-context proves the format is safe
- We prove our USE of the format is correct
- Mutation testing proves our tests aren't lying to us
