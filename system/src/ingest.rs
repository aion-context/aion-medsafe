// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bulk data ingestion with provenance sealing.
//!
//! Downloads and processes government bulk files (CSV, Excel) and
//! seals their provenance using aion-context manifests.

use std::path::Path;

use crate::provenance;

/// Run the ingestion command: read raw file, seal provenance.
pub fn run(file: &Path, source: &str, output: Option<&Path>) -> anyhow::Result<()> {
    if !file.exists() {
        anyhow::bail!("Source file not found: {}", file.display());
    }

    // Load the persisted signing key (created during `init`)
    let signing_key = provenance::load_signing_key()?;

    let file_size = std::fs::metadata(file)?.len();
    tracing::info!(
        event = "ingestion_started",
        source = source,
        file = %file.display(),
        size = file_size,
    );

    // Compute BLAKE3 hash of raw file
    let raw_bytes = std::fs::read(file)?;
    let hash = blake3::hash(&raw_bytes);

    tracing::info!(
        event = "file_hashed",
        source = source,
        blake3 = %hash,
        size = file_size,
    );

    // Determine output path for provenance manifest
    let default_output = Path::new("provenance").join(format!("{source}.aion"));
    let output_path = output.unwrap_or(&default_output);

    // Seal provenance
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manifest_hash = provenance::seal(file, source, output_path, &signing_key)?;

    println!("✓ Ingested: {}", file.display());
    println!("  Source: {source}");
    println!("  Size: {} bytes", file_size);
    println!("  BLAKE3: {hash}");
    println!("  Manifest: {}", output_path.display());
    println!("  Manifest hash: {}", hex::encode(manifest_hash));

    Ok(())
}
