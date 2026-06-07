// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sealed release attestation — supply-chain integrity for the binary itself.
//!
//! Produces an in-toto/SLSA-style provenance statement for a built binary (its
//! BLAKE3 digest, the package + version, the dependency lock digest, the source
//! commit, and the build time) and seals it into a `.aion`. This makes "which
//! exact binary produced these sealed signals, and from what dependencies?"
//! independently verifiable — the last link in the chain of custody.

use std::path::Path;
use std::process::Command;

use chrono::Utc;
use serde_json::Value;

use crate::provenance;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build the in-toto/SLSA provenance statement (pure — testable).
pub fn build_attestation(
    binary_name: &str,
    binary_blake3: &str,
    lock_blake3: Option<&str>,
    git_commit: &str,
    finished_on: &str,
) -> Value {
    serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": binary_name,
            "digest": { "blake3": binary_blake3 }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://github.com/aion-context/aion-medsafe/cargo-release",
                "externalParameters": { "package": PKG_NAME, "version": PKG_VERSION },
                "resolvedDependencies": [{
                    "uri": "Cargo.lock",
                    "digest": { "blake3": lock_blake3.unwrap_or("absent") }
                }]
            },
            "runDetails": {
                "builder": { "id": "aion-medsafe/release" },
                "metadata": { "finishedOn": finished_on, "sourceCommit": git_commit }
            }
        }
    })
}

fn git_commit() -> String {
    if let Ok(c) = std::env::var("GIT_COMMIT") {
        if !c.trim().is_empty() {
            return c.trim().to_string();
        }
    }
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Attest and seal a release binary.
pub fn run(binary: &Path, output: Option<&Path>) -> anyhow::Result<()> {
    if !binary.exists() {
        anyhow::bail!(
            "binary not found: {} (run `cargo build --release` first)",
            binary.display()
        );
    }
    let binary_bytes = std::fs::read(binary)?;
    let binary_blake3 = hex::encode(blake3::hash(&binary_bytes).as_bytes());
    let binary_name = binary
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "binary".to_string());

    let lock_path = Path::new("Cargo.lock");
    let lock_blake3 = std::fs::read(lock_path)
        .ok()
        .map(|b| hex::encode(blake3::hash(&b).as_bytes()));

    let finished_on = Utc::now().to_rfc3339();
    let commit = git_commit();
    let attestation = build_attestation(
        &binary_name,
        &binary_blake3,
        lock_blake3.as_deref(),
        &commit,
        &finished_on,
    );

    let out_path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new("release").join(format!("{PKG_NAME}_{PKG_VERSION}.aion")));
    let signing_key = provenance::load_signing_key()?;
    let sealed = provenance::seal_payload(
        &out_path,
        format!("{attestation}\n").as_bytes(),
        &signing_key,
        &format!("Release attestation {PKG_NAME} v{PKG_VERSION}"),
    )?;

    println!("✓ Release attestation sealed");
    println!("  Binary: {binary_name}  (BLAKE3 {binary_blake3})");
    println!("  Package: {PKG_NAME} v{PKG_VERSION}  commit {commit}");
    if let Some(l) = &lock_blake3 {
        println!("  Cargo.lock BLAKE3: {l}");
    }
    println!(
        "  Sealed: {} (manifest {})",
        out_path.display(),
        hex::encode(sealed)
    );
    println!(
        "  Verify: aion-medsafe provenance --manifest {}",
        out_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestation_has_subject_digest_and_provenance_shape() {
        let a = build_attestation(
            "aion-medsafe",
            "deadbeef",
            Some("cafef00d"),
            "abc123",
            "2026-06-07T00:00:00Z",
        );
        assert_eq!(a["subject"][0]["digest"]["blake3"], "deadbeef");
        assert_eq!(a["predicateType"], "https://slsa.dev/provenance/v1");
        assert_eq!(
            a["predicate"]["buildDefinition"]["resolvedDependencies"][0]["digest"]["blake3"],
            "cafef00d"
        );
        assert_eq!(
            a["predicate"]["runDetails"]["metadata"]["sourceCommit"],
            "abc123"
        );
    }

    #[test]
    fn missing_lock_marked_absent() {
        let a = build_attestation("b", "h", None, "c", "t");
        assert_eq!(
            a["predicate"]["buildDefinition"]["resolvedDependencies"][0]["digest"]["blake3"],
            "absent"
        );
    }
}
