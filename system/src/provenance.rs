// SPDX-License-Identifier: MIT OR Apache-2.0
//! Data provenance — seal and verify bulk data source ingestions.
//!
//! Every raw file we ingest gets an aion-context manifest that records:
//! - The BLAKE3 hash of the raw file
//! - The file size
//! - The source identifier
//! - A timestamp
//! - A signature by the pipeline's signing key
//!
//! Later, anyone with the registry can verify that the data we based
//! our risk signals on matches exactly what the government published.

use aion_context::crypto::{SigningKey, VerifyingKey};
use aion_context::key_registry::KeyRegistry;
use aion_context::manifest::{sign_manifest, ArtifactManifestBuilder};
use aion_context::operations::{
    commit_version, init_file, show_current_rules, verify_file, CommitOptions, InitOptions,
};
use aion_context::types::AuthorId;
use std::path::{Path, PathBuf};

use crate::error::{MedsafeError, Result};

/// Pipeline automation author ID (range 80001-80009)
const PIPELINE_AUTHOR: u64 = 80001;

/// Default path for the pipeline signing key (seed bytes).
const DEFAULT_KEY_PATH: &str = ".aion/pipeline.key";

/// On-disk path for an author's private signing key (seed bytes). The pipeline
/// automation key keeps its legacy path; every other author (analysts 80010+,
/// legal 80020+, admin 80030+) gets `.aion/author_<id>.key`. All `*.key` files
/// are gitignored — only public keys live in the committed registry.
pub fn author_key_path(author_id: u64) -> PathBuf {
    if author_id == PIPELINE_AUTHOR {
        PathBuf::from(DEFAULT_KEY_PATH)
    } else {
        PathBuf::from(format!(".aion/author_{author_id}.key"))
    }
}

/// Default path for the key registry (public keys only — safe to commit).
pub const DEFAULT_REGISTRY_PATH: &str = ".aion/medsafe.registry.json";

/// Load registry from disk using aion-context's trusted JSON format.
pub fn load_registry(registry_path: &Path) -> Result<KeyRegistry> {
    if !registry_path.exists() {
        return Err(MedsafeError::RegistryNotFound {
            path: registry_path.to_path_buf(),
        });
    }
    let json = std::fs::read_to_string(registry_path)?;
    let registry = KeyRegistry::from_trusted_json(&json).map_err(|e| MedsafeError::ParseError {
        source_name: registry_path.display().to_string(),
        reason: e.to_string(),
    })?;
    Ok(registry)
}

/// Seal arbitrary bytes into a signed `.aion` file whose payload IS the data.
///
/// This is the "sealed payload" pattern used for governance artifacts — the
/// detection policy, the Trust Graph, and signal output — where the data must
/// be verifiable in-band (via the four aion-context guarantees) rather than
/// through a side-car manifest. Contrast with [`seal`], which records only a
/// metadata manifest for a large external raw file.
///
/// Returns the BLAKE3 digest of the payload as proof of what was sealed.
pub fn seal_payload(
    output_path: &Path,
    payload: &[u8],
    signing_key: &SigningKey,
    message: &str,
) -> Result<[u8; 32]> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let author = AuthorId::new(PIPELINE_AUTHOR);
    let options = InitOptions {
        author_id: author,
        signing_key,
        message,
        timestamp: None,
    };
    init_file(output_path, payload, &options)?;

    let digest = blake3::hash(payload);
    tracing::info!(
        event = "payload_sealed",
        file = %output_path.display(),
        size = payload.len(),
        blake3 = %digest,
    );
    Ok(*digest.as_bytes())
}

/// Append a new sealed version to a `.aion` (or initialize it on first write).
///
/// For mutable governance logs — e.g. the human review-decision store — where
/// the aion-context hash chain across versions IS the tamper-evident audit
/// trail. Each call commits the full current payload as a new version bound to
/// its parent by `parent_hash`.
pub fn commit_payload(
    path: &Path,
    payload: &[u8],
    author_id: u64,
    signing_key: &SigningKey,
    registry: &KeyRegistry,
    message: &str,
) -> Result<()> {
    let author = AuthorId::new(author_id);
    if path.exists() {
        let options = CommitOptions {
            author_id: author,
            signing_key,
            message,
            timestamp: None,
        };
        commit_version(path, payload, &options, registry)?;
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = InitOptions {
            author_id: author,
            signing_key,
            message,
            timestamp: None,
        };
        init_file(path, payload, &options)?;
    }
    tracing::info!(
        event = "payload_committed",
        file = %path.display(),
        size = payload.len(),
    );
    Ok(())
}

/// Verify a sealed `.aion` (all four guarantees) and return its payload bytes.
///
/// This is the verification half of the "policy loop": it REFUSES to return
/// any data unless the file's structure, integrity hash, hash chain, and
/// signatures all verify against the registry. Trust is never cached — the
/// file is re-read and re-verified on every call.
pub fn load_verified_payload(aion_path: &Path, registry: &KeyRegistry) -> Result<Vec<u8>> {
    if !aion_path.exists() {
        return Err(MedsafeError::SourceNotFound {
            path: aion_path.to_path_buf(),
        });
    }

    // Pre-flight bounds check BEFORE handing the file to verify_file. This
    // defuses an aion-context 1.0 DoS where corrupted header counts/lengths
    // drive an unbounded allocation that aborts the process. See
    // preflight_header_bounds for the full rationale.
    let file_bytes = std::fs::read(aion_path)?;
    preflight_header_bounds(&file_bytes, aion_path)?;

    let report = verify_file(aion_path, registry).map_err(|e| MedsafeError::ProvenanceFailed {
        path: aion_path.to_path_buf(),
        reason: e.to_string(),
    })?;

    if !report.is_valid {
        tracing::error!(
            event = "provenance_verification_failed",
            file = %aion_path.display(),
            structure = report.structure_valid,
            integrity = report.integrity_hash_valid,
            chain = report.hash_chain_valid,
            signatures = report.signatures_valid,
        );
        return Err(MedsafeError::ProvenanceFailed {
            path: aion_path.to_path_buf(),
            reason: format!(
                "structure={} integrity={} chain={} signatures={}: {:?}",
                report.structure_valid,
                report.integrity_hash_valid,
                report.hash_chain_valid,
                report.signatures_valid,
                report.errors,
            ),
        });
    }

    let payload = show_current_rules(aion_path).map_err(|e| MedsafeError::ProvenanceFailed {
        path: aion_path.to_path_buf(),
        reason: format!("payload extraction failed: {e}"),
    })?;

    tracing::info!(
        event = "payload_verified",
        file = %aion_path.display(),
        version_count = report.version_count,
        size = payload.len(),
    );
    Ok(payload)
}

/// Defuse a known aion-context 1.0 denial-of-service before verification.
///
/// `verify_file` reads entry counts and section lengths straight from the file
/// header and uses them as `Vec::with_capacity` sizes (operations.rs
/// `collect_versions` / `collect_signatures`). A corrupted header — even a
/// single flipped byte in a count field — can therefore drive an unbounded
/// allocation that aborts the process (SIGABRT) instead of returning an error,
/// turning "tamper → refuse" into "tamper → crash" (a DoS on the verifier).
///
/// We pre-validate using aion-context's OWN zero-copy header parser
/// (`AionParser::new` parses only the fixed-size header — no count-driven
/// allocation), so this is not hand-rolled `.aion` parsing. We refuse any
/// header whose sections fall outside the file or whose entry counts exceed the
/// file length (each on-disk entry is at least one byte). This bounds the
/// worst-case allocation to the file size rather than `u64::MAX`, converting the
/// abort into a clean `Err`.
fn preflight_header_bounds(file_bytes: &[u8], path: &Path) -> Result<()> {
    use aion_context::parser::AionParser;

    let parser = AionParser::new(file_bytes).map_err(|e| MedsafeError::ProvenanceFailed {
        path: path.to_path_buf(),
        reason: format!("header parse failed: {e}"),
    })?;
    let header = parser.header();
    let file_len = file_bytes.len() as u64;

    let refuse = |reason: String| -> Result<()> {
        tracing::error!(event = "provenance_preflight_refused", file = %path.display(), reason = %reason);
        Err(MedsafeError::ProvenanceFailed {
            path: path.to_path_buf(),
            reason,
        })
    };

    let sections = [
        (
            "encrypted_rules",
            header.encrypted_rules_offset,
            header.encrypted_rules_length,
        ),
        (
            "string_table",
            header.string_table_offset,
            header.string_table_length,
        ),
    ];
    for (name, offset, length) in sections {
        if offset > file_len || length > file_len || offset.saturating_add(length) > file_len {
            return refuse(format!(
                "section {name} out of bounds (offset {offset}, length {length}, file {file_len})"
            ));
        }
    }

    let counts = [
        ("version_chain", header.version_chain_count),
        ("signatures", header.signatures_count),
        ("audit_trail", header.audit_trail_count),
    ];
    for (name, count) in counts {
        if count > file_len {
            return refuse(format!(
                "{name}_count {count} exceeds file length {file_len} — refusing to allocate"
            ));
        }
    }

    Ok(())
}

/// Load the pipeline signing key from disk.
pub fn load_signing_key() -> anyhow::Result<SigningKey> {
    load_signing_key_for(PIPELINE_AUTHOR)
}

/// Load a specific author's signing key from disk.
pub fn load_signing_key_for(author_id: u64) -> anyhow::Result<SigningKey> {
    let key_path = author_key_path(author_id);
    if !key_path.exists() {
        let hint = if author_id == PIPELINE_AUTHOR {
            "Run `aion-medsafe init` first.".to_string()
        } else {
            format!("Run `aion-medsafe enroll-analyst --author {author_id}` first.")
        };
        anyhow::bail!(
            "Signing key for author {author_id} not found at {}. {hint}",
            key_path.display()
        );
    }
    let key_bytes = std::fs::read(&key_path)?;
    let key = SigningKey::from_bytes(&key_bytes)?;
    Ok(key)
}

/// Enroll a non-pipeline author (analyst/legal/admin) into the registry.
///
/// Generates a keypair, registers the PUBLIC key in the (committed) registry,
/// and writes the PRIVATE key to `.aion/author_<id>.key` (gitignored). After
/// enrollment, that author can sign decisions whose signatures verify against
/// the registry — making each decision cryptographically attributable.
pub fn enroll_author(registry_path: &Path, author_id: u64) -> anyhow::Result<()> {
    if (PIPELINE_AUTHOR..PIPELINE_AUTHOR + 9).contains(&author_id) {
        anyhow::bail!(
            "author {author_id} is in the pipeline-automation range (80001-80009); \
             reviewers must be analyst/legal/admin (80010+)"
        );
    }
    let key_path = author_key_path(author_id);
    if key_path.exists() {
        anyhow::bail!(
            "author {author_id} already has a key at {}; refusing to overwrite",
            key_path.display()
        );
    }

    let mut registry = load_registry(registry_path)?;
    let key = SigningKey::generate();
    registry.register_author(
        AuthorId::new(author_id),
        key.verifying_key(),
        key.verifying_key(),
        0,
    )?;
    std::fs::write(registry_path, registry.to_trusted_json()?)?;

    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&key_path, key.to_bytes())?;

    tracing::info!(event = "author_enrolled", author_id, key = %key_path.display());
    Ok(())
}

/// Generate a keypair offline: write the PRIVATE key to `out`, return the PUBLIC
/// key as hex. The public key is then registered centrally via
/// [`register_external_author`] — separating key generation (which can happen on
/// an analyst's machine / HSM) from registration (which the admin performs).
pub fn keygen(out: &Path) -> anyhow::Result<String> {
    if out.exists() {
        anyhow::bail!("refusing to overwrite existing key at {}", out.display());
    }
    let key = SigningKey::generate();
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(out, key.to_bytes())?;
    Ok(hex::encode(key.verifying_key().to_bytes()))
}

/// Register an externally-held author by PUBLIC KEY only (hex-encoded 32-byte
/// Ed25519). No private key is generated or stored here — the holder keeps it
/// (HSM, keyring, another machine). This lets the registry admit a signer whose
/// past/future logs then verify, without the system ever touching their key.
pub fn register_external_author(
    registry_path: &Path,
    author_id: u64,
    pubkey_hex: &str,
) -> anyhow::Result<()> {
    if (PIPELINE_AUTHOR..PIPELINE_AUTHOR + 9).contains(&author_id) {
        anyhow::bail!(
            "author {author_id} is in the pipeline-automation range (80001-80009); \
             reviewers must be analyst/legal/admin (80010+)"
        );
    }
    let bytes = hex::decode(pubkey_hex.trim())
        .map_err(|e| anyhow::anyhow!("public key must be hex: {e}"))?;
    if bytes.len() != 32 {
        anyhow::bail!(
            "public key must be 32 bytes (64 hex chars), got {}",
            bytes.len()
        );
    }
    let verifying_key = VerifyingKey::from_bytes(&bytes)?;

    let mut registry = load_registry(registry_path)?;
    if registry.master_key(AuthorId::new(author_id)).is_some() {
        anyhow::bail!("author {author_id} is already registered");
    }
    registry.register_author(AuthorId::new(author_id), verifying_key, verifying_key, 0)?;
    std::fs::write(registry_path, registry.to_trusted_json()?)?;

    tracing::info!(event = "external_author_registered", author_id);
    Ok(())
}

/// Initialize the key registry for AION-MEDSAFE.
///
/// Creates a registry with the pipeline's signing key registered.
/// In production, this would be backed by OS keyring or HSM.
pub fn init_registry(registry_path: &Path) -> anyhow::Result<()> {
    let key = SigningKey::generate();
    let mut registry = KeyRegistry::new();
    let author = AuthorId::new(PIPELINE_AUTHOR);

    registry.register_author(author, key.verifying_key(), key.verifying_key(), 0)?;

    // Serialize registry using aion-context's own trusted JSON format
    let registry_json = registry.to_trusted_json()?;
    if let Some(parent) = registry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(registry_path, &registry_json)?;

    // Persist the signing key so ingest/commit commands can reuse it
    let key_path = Path::new(DEFAULT_KEY_PATH);
    std::fs::write(key_path, key.to_bytes())?;

    tracing::info!(
        event = "registry_created",
        author_id = PIPELINE_AUTHOR,
        path = %registry_path.display(),
    );

    Ok(())
}

/// Seal provenance for a bulk data file ingestion.
///
/// Computes BLAKE3 hash of the raw file and creates a signed manifest.
pub fn seal(
    raw_file_path: &Path,
    source_id: &str,
    output_path: &Path,
    signing_key: &SigningKey,
) -> anyhow::Result<[u8; 32]> {
    let raw_bytes = std::fs::read(raw_file_path)?;
    let file_size = raw_bytes.len();

    // Build artifact manifest
    let mut builder = ArtifactManifestBuilder::new();
    let artifact_name = format!("{}_{}", source_id, chrono::Utc::now().format("%Y-%m-%d"));
    // Single artifact: the returned handle is only needed for by-index lookup,
    // which seal() never does (it uses manifest.manifest_id()). Bind to _handle
    // per .claude/rules/aion-context-patterns.md.
    let _handle = builder.add(&artifact_name, &raw_bytes);
    let manifest = builder.build();
    let manifest_id = *manifest.manifest_id();

    // Sign the manifest
    let author = AuthorId::new(PIPELINE_AUTHOR);
    let _sig = sign_manifest(&manifest, author, signing_key);

    // Create .aion file with the manifest metadata as rules payload
    let manifest_yaml = format!(
        "source_id: {source_id}\n\
         file: {}\n\
         size: {file_size}\n\
         blake3: {}\n\
         ingested_at: {}\n",
        raw_file_path.display(),
        hex::encode(manifest_id),
        chrono::Utc::now().to_rfc3339(),
    );

    let options = InitOptions {
        author_id: author,
        signing_key,
        message: &format!("Ingest {source_id}"),
        timestamp: None,
    };

    init_file(output_path, manifest_yaml.as_bytes(), &options)?;

    tracing::info!(
        event = "provenance_sealed",
        source_id = source_id,
        file = %raw_file_path.display(),
        size = file_size,
        manifest_hash = %hex::encode(manifest_id),
    );

    Ok(manifest_id)
}

/// Verify provenance of a data file against its manifest.
pub fn verify(manifest_path: &Path, data_file_path: &Path) -> anyhow::Result<()> {
    let registry_path = Path::new(".aion/medsafe.registry.json");
    let registry = load_registry(registry_path)?;

    // Verify the .aion file integrity
    let report = verify_file(manifest_path, &registry)?;
    if !report.is_valid {
        tracing::error!(
            event = "provenance_verification_failed",
            manifest = %manifest_path.display(),
            errors = ?report.errors,
        );
        anyhow::bail!("Provenance verification failed: {:?}", report.errors);
    }

    // Verify the data file hash matches what's in the manifest
    let data_bytes = std::fs::read(data_file_path)?;
    let actual_hash = blake3::hash(&data_bytes);

    tracing::info!(
        event = "provenance_verified",
        manifest = %manifest_path.display(),
        data_file = %data_file_path.display(),
        hash = %actual_hash,
        structure_valid = report.structure_valid,
        integrity_valid = report.integrity_hash_valid,
        chain_valid = report.hash_chain_valid,
        signatures_valid = report.signatures_valid,
    );

    println!("✓ Provenance verified");
    println!("  File: {}", data_file_path.display());
    println!("  BLAKE3: {}", actual_hash);
    println!("  Manifest: {}", manifest_path.display());
    println!("  Chain valid: {}", report.hash_chain_valid);
    println!("  Signatures valid: {}", report.signatures_valid);

    Ok(())
}

/// Show provenance information for a manifest.
pub fn show(manifest_path: &Path) -> anyhow::Result<()> {
    let registry_path = Path::new(".aion/medsafe.registry.json");
    let registry = load_registry(registry_path)?;

    let report = verify_file(manifest_path, &registry)?;

    println!("Provenance: {}", manifest_path.display());
    println!("  Versions: {}", report.version_count);
    println!("  Valid: {}", report.is_valid);
    println!("  Structure: {}", report.structure_valid);
    println!("  Integrity: {}", report.integrity_hash_valid);
    println!("  Hash chain: {}", report.hash_chain_valid);
    println!("  Signatures: {}", report.signatures_valid);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_context::types::AuthorId;
    use proptest::prelude::*;

    fn registry_and_key() -> (KeyRegistry, SigningKey) {
        let key = SigningKey::generate();
        let mut registry = KeyRegistry::new();
        registry
            .register_author(
                AuthorId::new(PIPELINE_AUTHOR),
                key.verifying_key(),
                key.verifying_key(),
                0,
            )
            .expect("register author");
        (registry, key)
    }

    #[test]
    fn seal_payload_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("p.aion");
        let (registry, key) = registry_and_key();
        let payload = b"version: 1\nrules: yes\n";

        seal_payload(&path, payload, &key, "seal").expect("seal");
        let loaded = load_verified_payload(&path, &registry).expect("load");
        assert_eq!(loaded, payload);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// The guarded loader must NEVER abort and must refuse ANY single-byte
        /// corruption of a sealed file — this is the regression test for the
        /// aion-context unbounded-allocation DoS (see preflight_header_bounds).
        #[test]
        fn guarded_loader_refuses_any_single_byte_tamper(
            payload in prop::collection::vec(any::<u8>(), 16..4_000),
            flip_pct in 0.0f64..1.0,
        ) {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("p.aion");
            let (registry, key) = registry_and_key();
            seal_payload(&path, &payload, &key, "seal").expect("seal");

            // Sanity: the untouched file loads and round-trips.
            prop_assert_eq!(load_verified_payload(&path, &registry).expect("load clean"), payload.clone());

            // Flip one byte anywhere in the sealed file.
            let mut bytes = std::fs::read(&path).expect("read");
            let offset = ((flip_pct * (bytes.len() - 1) as f64) as usize).min(bytes.len() - 1);
            bytes[offset] ^= 0x01;
            std::fs::write(&path, &bytes).expect("write");

            // Must return Err (refuse) — and crucially must not abort the process.
            prop_assert!(load_verified_payload(&path, &registry).is_err(),
                "tamper at offset {} was not refused", offset);
        }
    }
}
