// SPDX-License-Identifier: MIT OR Apache-2.0
//! Policy-gated signal generation.
//!
//! Detection rules live in a signed .aion file. Before computing any risk
//! signals, the system verifies the policy file's integrity and signature
//! chain. If verification fails, the system REFUSES to generate signals —
//! it will not act on potentially tampered rules.
//!
//! This is the aion-context "policy loop" pattern applied to fraud detection.

use aion_context::key_registry::KeyRegistry;
use aion_context::operations::{show_current_rules, verify_file};
use serde::Deserialize;
use std::path::Path;

use crate::error::{MedsafeError, Result};

/// A risk signal definition from the policy file.
#[derive(Debug, Clone, Deserialize)]
pub struct SignalDefinition {
    pub severity: f64,
    pub description: String,
    pub requires_human_review: bool,
    #[serde(default)]
    pub escalation: Option<String>,
}

/// Detection policy loaded from a verified .aion file.
#[derive(Debug, Clone, Deserialize)]
pub struct DetectionPolicy {
    pub version: String,
    pub effective_date: String,
    pub risk_signals: std::collections::HashMap<String, SignalDefinition>,
    pub thresholds: Thresholds,
    pub jurisdictions: Jurisdictions,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Thresholds {
    pub minimum_confidence_for_alert: f64,
    pub maximum_days_lookback: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Jurisdictions {
    pub primary: String,
    pub scope: String,
}

impl DetectionPolicy {
    /// Load and verify a detection policy from a signed .aion file.
    ///
    /// This function performs FULL verification before extracting rules.
    /// If any of the four guarantees (structure, integrity, hash chain,
    /// signatures) fails, it returns an error and REFUSES to provide
    /// the policy — the system must not act on tampered rules.
    pub fn load_verified(policy_path: &Path, registry: &KeyRegistry) -> Result<Self> {
        // Verify the .aion file — all four guarantees must hold
        let report = verify_file(policy_path, registry).map_err(|e| {
            MedsafeError::PolicyIntegrityFailed {
                reasons: vec![e.to_string()],
            }
        })?;

        if !report.is_valid {
            tracing::error!(
                event = "policy_verification_failed",
                path = %policy_path.display(),
                structure = report.structure_valid,
                integrity = report.integrity_hash_valid,
                chain = report.hash_chain_valid,
                signatures = report.signatures_valid,
            );
            return Err(MedsafeError::PolicyIntegrityFailed {
                reasons: report.errors.clone(),
            });
        }

        tracing::info!(
            event = "policy_verified",
            path = %policy_path.display(),
            version_count = report.version_count,
        );

        // Extract the plaintext rules from the verified file
        let rules_bytes =
            show_current_rules(policy_path).map_err(|e| MedsafeError::PolicyIntegrityFailed {
                reasons: vec![format!("Failed to extract rules: {e}")],
            })?;

        // Parse YAML rules into our policy struct
        let policy: DetectionPolicy =
            serde_yaml::from_slice(&rules_bytes).map_err(|e| MedsafeError::ParseError {
                source_name: policy_path.display().to_string(),
                reason: e.to_string(),
            })?;

        tracing::info!(
            event = "policy_loaded",
            version = %policy.version,
            effective_date = %policy.effective_date,
            signal_count = policy.risk_signals.len(),
            jurisdiction = %policy.jurisdictions.primary,
        );

        Ok(policy)
    }

    /// Check if a signal type is defined in the policy.
    pub fn has_signal(&self, signal_type: &str) -> bool {
        self.risk_signals.contains_key(signal_type)
    }

    /// Get the severity for a signal type, or None if not defined.
    pub fn severity(&self, signal_type: &str) -> Option<f64> {
        self.risk_signals.get(signal_type).map(|s| s.severity)
    }

    /// Check if a signal requires human review per policy.
    pub fn requires_review(&self, signal_type: &str) -> bool {
        self.risk_signals
            .get(signal_type)
            .map_or(true, |s| s.requires_human_review) // default: require review
    }
}
