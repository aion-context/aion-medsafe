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

use aion_context::crypto::SigningKey;
use aion_context::key_registry::KeyRegistry;
use aion_context::manifest::{sign_manifest, ArtifactManifestBuilder};
use aion_context::operations::{init_file, verify_file, InitOptions};
use aion_context::types::AuthorId;
use std::path::Path;

/// Pipeline automation author ID (range 80001-80009)
const PIPELINE_AUTHOR: u64 = 80001;

/// Default path for the signing key (seed bytes).
const DEFAULT_KEY_PATH: &str = ".aion/pipeline.key";

/// Load registry from disk using aion-context's trusted JSON format.
fn load_registry(registry_path: &Path) -> anyhow::Result<KeyRegistry> {
    let json = std::fs::read_to_string(registry_path)?;
    let registry = KeyRegistry::from_trusted_json(&json)?;
    Ok(registry)
}

/// Load the pipeline signing key from disk.
pub fn load_signing_key() -> anyhow::Result<SigningKey> {
    let key_path = Path::new(DEFAULT_KEY_PATH);
    if !key_path.exists() {
        anyhow::bail!(
            "Signing key not found at {}. Run `aion-medsafe init` first.",
            key_path.display()
        );
    }
    let key_bytes = std::fs::read(key_path)?;
    let key = SigningKey::from_bytes(&key_bytes)?;
    Ok(key)
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
