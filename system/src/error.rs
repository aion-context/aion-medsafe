// SPDX-License-Identifier: MIT OR Apache-2.0
//! Error types for AION-MEDSAFE.
//!
//! Tiger Style: every fallible operation returns Result<T, MedsafeError>.
//! No panics, no unwrap, no expect.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MedsafeError {
    #[error("Policy integrity check failed: {reasons:?}")]
    PolicyIntegrityFailed { reasons: Vec<String> },

    #[error("Provenance verification failed for {path}: {reason}")]
    ProvenanceFailed { path: PathBuf, reason: String },

    #[error("Data source not found: {path}")]
    SourceNotFound { path: PathBuf },

    #[error("Registry not initialized at {path}")]
    RegistryNotFound { path: PathBuf },

    #[error("Signing key not available for author {author_id}")]
    KeyNotAvailable { author_id: u64 },

    #[error("Parse error in {source_name}: {reason}")]
    ParseError { source_name: String, reason: String },

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Aion context error: {0}")]
    AionContext(#[from] aion_context::AionError),
}

pub type Result<T> = std::result::Result<T, MedsafeError>;
