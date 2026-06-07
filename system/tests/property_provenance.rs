// SPDX-License-Identifier: MIT OR Apache-2.0
//! Property-based tests for provenance sealing and verification.
//!
//! These tests prove invariants hold for ALL inputs, not just hand-picked examples.
//! A failure here means a real bug — either in the code or in our understanding
//! of the invariants.

use aion_context::crypto::SigningKey;
use aion_context::key_registry::KeyRegistry;
use aion_context::operations::{
    commit_version, init_file, show_current_rules, verify_file, CommitOptions, InitOptions,
};
use aion_context::types::AuthorId;
use proptest::prelude::*;
use std::path::Path;

/// Helper: create a registry and key pair for testing.
fn test_registry() -> (KeyRegistry, SigningKey, AuthorId) {
    let key = SigningKey::generate();
    let mut registry = KeyRegistry::new();
    let author = AuthorId::new(80001);
    registry
        .register_author(author, key.verifying_key(), key.verifying_key(), 0)
        .expect("test registry setup");
    (registry, key, author)
}

/// Helper: seal a file and return (path, registry) for verification.
fn seal_test_file(
    dir: &tempfile::TempDir,
    payload: &[u8],
    filename: &str,
) -> (std::path::PathBuf, KeyRegistry) {
    let (registry, key, author) = test_registry();
    let path = dir.path().join(filename);
    let options = InitOptions {
        author_id: author,
        signing_key: &key,
        message: "property test genesis",
        timestamp: None,
    };
    init_file(&path, payload, &options).expect("seal_test_file init");
    (path, registry)
}

// ============================================================================
// PROPERTY: Seal/Verify Round-Trip
// For ANY valid payload, seal → verify = VALID
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn prop_seal_verify_roundtrip(
        payload in prop::collection::vec(any::<u8>(), 1..10_000),
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (path, registry) = seal_test_file(&dir, &payload, "roundtrip.aion");

        let report = verify_file(&path, &registry).expect("verify_file");
        prop_assert!(report.is_valid,
            "Sealed file must verify as valid. Errors: {:?}", report.errors);
        prop_assert!(report.structure_valid);
        prop_assert!(report.integrity_hash_valid);
        prop_assert!(report.hash_chain_valid);
        prop_assert!(report.signatures_valid);
        prop_assert_eq!(report.version_count, 1);
    }
}

// ============================================================================
// PROPERTY: Single-Byte Tamper Detection
// For ANY sealed file, flipping ANY single byte → verify = INVALID
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn prop_single_byte_flip_always_detected(
        payload in prop::collection::vec(any::<u8>(), 100..5_000),
        flip_pct in 0.01f64..0.99,
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (path, registry) = seal_test_file(&dir, &payload, "tamper.aion");

        // Read the sealed file
        let mut bytes = std::fs::read(&path).expect("read sealed file");
        let file_len = bytes.len();

        // Flip one byte at a random offset (avoid offset 0 which is magic bytes)
        let offset = (flip_pct * (file_len - 1) as f64) as usize;
        let offset = offset.max(4); // skip magic "AION" header bytes
        bytes[offset] ^= 0x01;
        std::fs::write(&path, &bytes).expect("write tampered file");

        // Verification MUST fail — at least one of the four guarantees breaks
        let report = verify_file(&path, &registry);
        match report {
            Ok(r) => prop_assert!(!r.is_valid,
                "Tampered file at offset {} must NOT verify as valid", offset),
            Err(_) => {} // parse failure is also acceptable (file too corrupted to parse)
        }
    }
}

// ============================================================================
// PROPERTY: Rules Extraction Round-Trip
// For ANY payload, seal → extract rules = original payload
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn prop_rules_extraction_roundtrip(
        payload in prop::collection::vec(any::<u8>(), 1..8_000),
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (path, _registry) = seal_test_file(&dir, &payload, "extract.aion");

        let extracted = show_current_rules(&path).expect("show_current_rules");
        prop_assert_eq!(&extracted, &payload,
            "Extracted rules must exactly match original payload");
    }
}

// ============================================================================
// PROPERTY: Version Chain Integrity
// Committing N versions → verify still passes, version_count = N
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_version_chain_grows_correctly(
        payloads in prop::collection::vec(
            prop::collection::vec(any::<u8>(), 10..1_000),
            2..6  // 2-5 additional versions after genesis
        ),
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (registry, key, author) = test_registry();
        let path = dir.path().join("chain.aion");

        // Genesis
        let genesis_payload = b"genesis rules";
        let options = InitOptions {
            author_id: author,
            signing_key: &key,
            message: "v1",
            timestamp: None,
        };
        init_file(&path, genesis_payload, &options).expect("init");

        // Commit additional versions
        let expected_versions = payloads.len() as u64 + 1; // +1 for genesis
        for (i, payload) in payloads.iter().enumerate() {
            let commit_opts = CommitOptions {
                author_id: author,
                signing_key: &key,
                message: &format!("v{}", i + 2),
                timestamp: None,
            };
            commit_version(&path, payload, &commit_opts, &registry).expect("commit");
        }

        // Verify the full chain
        let report = verify_file(&path, &registry).expect("verify");
        prop_assert!(report.is_valid,
            "Chain with {} versions must verify. Errors: {:?}",
            expected_versions, report.errors);
        prop_assert_eq!(report.version_count, expected_versions);
    }
}

// ============================================================================
// PROPERTY: Hash Determinism
// BLAKE3 of same bytes is always the same
// ============================================================================
proptest! {
    #[test]
    fn prop_blake3_is_deterministic(
        data in prop::collection::vec(any::<u8>(), 0..50_000),
    ) {
        let h1 = blake3::hash(&data);
        let h2 = blake3::hash(&data);
        prop_assert_eq!(h1, h2, "BLAKE3 must be deterministic");
    }
}

// ============================================================================
// PROPERTY: Different Payloads Produce Different Hashes
// (collision resistance — probabilistic, but with overwhelming confidence)
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_blake3_collision_resistance(
        data1 in prop::collection::vec(any::<u8>(), 1..10_000),
        data2 in prop::collection::vec(any::<u8>(), 1..10_000),
    ) {
        // Only check when inputs differ
        prop_assume!(data1 != data2);
        let h1 = blake3::hash(&data1);
        let h2 = blake3::hash(&data2);
        prop_assert_ne!(h1, h2, "Different inputs should produce different hashes");
    }
}

// ============================================================================
// PROPERTY: Registry Serialization Round-Trip
// Save → load → save produces identical JSON
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_registry_roundtrip(
        author_id in 1u64..100_000,
    ) {
        let key = SigningKey::generate();
        let mut registry = KeyRegistry::new();
        let author = AuthorId::new(author_id);
        registry
            .register_author(author, key.verifying_key(), key.verifying_key(), 0)
            .expect("register");

        let json1 = registry.to_trusted_json().expect("serialize");
        let loaded = KeyRegistry::from_trusted_json(&json1).expect("deserialize");
        let json2 = loaded.to_trusted_json().expect("re-serialize");

        prop_assert_eq!(json1, json2, "Registry round-trip must be idempotent");
    }
}
