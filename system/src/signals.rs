// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk signal computation — policy-gated.
//!
//! This module REFUSES to compute signals unless the detection policy
//! passes full aion-context verification. Every signal generated
//! carries a reference to the verified policy version that authorized it.

use aion_context::key_registry::KeyRegistry;
use std::path::Path;

use crate::policy::DetectionPolicy;

/// Run the signal computation pipeline.
///
/// Flow:
/// 1. Load and verify registry
/// 2. Load and verify detection policy (.aion)
/// 3. Load Trust Graph export
/// 4. Compute signals per policy rules
/// 5. Filter by jurisdiction if specified
/// 6. Output signals with provenance references
pub fn run(
    policy_path: &Path,
    graph_path: &Path,
    jurisdiction: Option<&str>,
) -> anyhow::Result<()> {
    // Step 1: Load registry
    let registry_path = Path::new(".aion/medsafe.registry.json");
    if !registry_path.exists() {
        anyhow::bail!(
            "Registry not found at {}. Run `aion-medsafe init` first.",
            registry_path.display()
        );
    }
    let registry_json = std::fs::read_to_string(registry_path)?;
    let registry = KeyRegistry::from_trusted_json(&registry_json)?;

    // Step 2: Load and VERIFY detection policy
    // This is the critical gate — if policy is tampered, we refuse to act
    let policy = DetectionPolicy::load_verified(policy_path, &registry)?;

    tracing::info!(
        event = "signal_computation_authorized",
        policy_version = %policy.version,
        effective_date = %policy.effective_date,
        jurisdiction = jurisdiction.unwrap_or("national"),
    );

    // Step 3: Load Trust Graph
    if !graph_path.exists() {
        anyhow::bail!("Trust Graph not found at {}", graph_path.display());
    }

    // Step 4: Compute signals per policy
    // (In full implementation, this iterates the graph and applies each
    // signal definition from the policy)
    let signal_types: Vec<&str> = policy.risk_signals.keys().map(|s| s.as_str()).collect();
    tracing::info!(
        event = "computing_signals",
        signal_types = ?signal_types,
        jurisdiction = jurisdiction.unwrap_or("national"),
        threshold = policy.thresholds.minimum_confidence_for_alert,
    );

    println!(
        "✓ Policy verified: v{} (effective {})",
        policy.version, policy.effective_date
    );
    println!("  Signal types: {}", signal_types.len());
    println!(
        "  Jurisdiction: {}",
        jurisdiction.unwrap_or(&policy.jurisdictions.primary)
    );
    println!(
        "  Min confidence: {}",
        policy.thresholds.minimum_confidence_for_alert
    );
    println!(
        "  Lookback: {} days",
        policy.thresholds.maximum_days_lookback
    );
    println!();
    println!("  Computing signals against Trust Graph...");

    // TODO: Implement full graph traversal and signal computation
    // For now, this proves the policy-gate pattern works

    Ok(())
}
