// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk signal computation — the full sealed chain of custody.
//!
//! Flow (every link verified or sealed):
//!   1. Load registry
//!   2. Verify + load the Trust Graph from its sealed `.aion`
//!   3. Verify + load the detection policy from its sealed `.aion`
//!   4. Compute signals over the verified graph (REFUSES on any failure)
//!   5. Filter by jurisdiction (handled in the detection engine)
//!   6. Apply the threshold + human-review gate (no autonomous accusation)
//!   7. Seal the signal output as its own `.aion` (evidence custody)

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::detection::{self, ComputedSignal, DetectionReport, ReviewCandidate};
use crate::graph::TrustGraph;
use crate::policy::DetectionPolicy;
use crate::provenance;

/// Run the signal computation pipeline and seal the output.
pub fn run(
    policy_path: &Path,
    graph_path: &Path,
    jurisdiction: Option<&str>,
    output: Option<&Path>,
) -> anyhow::Result<()> {
    // Step 1: registry
    let registry = provenance::load_registry(Path::new(provenance::DEFAULT_REGISTRY_PATH))?;

    // Step 2: verify + load the Trust Graph (sealed .aion payload)
    let graph = TrustGraph::load_verified(graph_path, &registry)?;

    // Step 3: verify + load the detection policy (the critical gate)
    let policy = DetectionPolicy::load_verified(policy_path, &registry)?;

    tracing::info!(
        event = "signal_computation_authorized",
        policy_version = %policy.version,
        effective_date = %policy.effective_date,
        jurisdiction = jurisdiction.unwrap_or("national"),
    );

    // Step 4-6: compute, jurisdiction filter, threshold + review gate
    let report = detection::compute(&graph, &policy, jurisdiction);

    // Entity-resolution review queue (sub-merge identity links for a human).
    let review = detection::review_queue(&graph, jurisdiction);

    // Step 7: seal the signal output as its own .aion
    let payload = serialize_report(&policy, &report, &review, jurisdiction);
    let output_path = resolve_output(output, jurisdiction);
    let signing_key = provenance::load_signing_key()?;
    let sealed = provenance::seal_payload(
        &output_path,
        payload.as_bytes(),
        &signing_key,
        &format!(
            "Signals {} (policy v{})",
            jurisdiction.unwrap_or("national"),
            policy.version
        ),
    )?;

    print_summary(
        &policy,
        &report,
        &review,
        jurisdiction,
        &output_path,
        &sealed,
    );
    Ok(())
}

/// Serialize the run as newline-delimited audit records: one run-meta line
/// followed by one line per computed signal.
fn serialize_report(
    policy: &DetectionPolicy,
    report: &DetectionReport,
    review: &[ReviewCandidate],
    jurisdiction: Option<&str>,
) -> String {
    let queued = report
        .signals
        .iter()
        .filter(|s| s.reason_code == "signal_queued_review")
        .count();

    let meta = serde_json::json!({
        "record": "run_meta",
        "agent": "provider_risk",
        "computed_at": Utc::now().to_rfc3339(),
        "policy_version": policy.version,
        "jurisdiction": jurisdiction.unwrap_or("national"),
        "entities_evaluated": report.entities_evaluated,
        "signal_count": report.signals.len(),
        "queued_for_review": queued,
        "below_threshold": report.signals.len() - queued,
        "not_computable": report.not_computable,
        "identity_review_candidates": review.len(),
    });

    let mut out = String::new();
    out.push_str(&meta.to_string());
    out.push('\n');
    for signal in &report.signals {
        // ComputedSignal is Serialize; serialization cannot fail for this shape.
        if let Ok(line) = serde_json::to_string(signal) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    // Identity-link review queue (reason_code = "identity_review_candidate").
    for candidate in review {
        if let Ok(line) = serde_json::to_string(candidate) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

fn resolve_output(output: Option<&Path>, jurisdiction: Option<&str>) -> PathBuf {
    match output {
        Some(p) => p.to_path_buf(),
        None => {
            let jur = jurisdiction.unwrap_or("national").to_lowercase();
            // Second precision so re-runs on the same day don't collide
            // (init_file refuses to overwrite an existing sealed file).
            let stamp = Utc::now().format("%Y-%m-%dT%H%M%SZ");
            Path::new("provenance").join(format!("signals_{jur}_{stamp}.aion"))
        }
    }
}

fn print_summary(
    policy: &DetectionPolicy,
    report: &DetectionReport,
    review: &[ReviewCandidate],
    jurisdiction: Option<&str>,
    output_path: &Path,
    sealed: &[u8; 32],
) {
    let queued = report
        .signals
        .iter()
        .filter(|s| s.reason_code == "signal_queued_review")
        .count();

    println!(
        "✓ Policy verified: v{} (effective {})",
        policy.version, policy.effective_date
    );
    println!(
        "  Jurisdiction: {}",
        jurisdiction.unwrap_or(&policy.jurisdictions.primary)
    );
    println!(
        "  Threshold: {}",
        policy.thresholds.minimum_confidence_for_alert
    );
    println!("  Entities evaluated: {}", report.entities_evaluated);
    println!();
    println!("  Signals generated: {}", report.signals.len());
    println!("    Queued for human review: {queued}");
    println!(
        "    Below threshold (archived): {}",
        report.signals.len() - queued
    );
    if !report.not_computable.is_empty() {
        println!(
            "    Not computable (insufficient data): {}",
            report.not_computable.join(", ")
        );
    }
    print_top_signals(report);

    println!();
    println!(
        "  Identity review queue (entity-resolution): {}",
        review.len()
    );
    print_top_candidates(review);

    println!();
    println!("  Sealed signal output: {}", output_path.display());
    println!("  Output manifest hash: {}", hex::encode(sealed));
}

/// Print up to five highest-confidence identity-link candidates for review.
fn print_top_candidates(review: &[ReviewCandidate]) {
    if review.is_empty() {
        return;
    }
    println!("  Top identity links to confirm/reject:");
    for candidate in review.iter().take(5) {
        println!(
            "    [{:.2}] {} ⇄ {}",
            candidate.confidence, candidate.name_a, candidate.name_b
        );
    }
}

/// Print up to five highest-confidence queued signals as a reviewer preview.
fn print_top_signals(report: &DetectionReport) {
    let mut queued: Vec<&ComputedSignal> = report
        .signals
        .iter()
        .filter(|s| s.reason_code == "signal_queued_review")
        .collect();
    queued.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    if queued.is_empty() {
        return;
    }
    println!();
    println!("  Top signals queued for review:");
    for signal in queued.iter().take(5) {
        println!(
            "    [{:.2}] {} — {} ({})",
            signal.confidence, signal.signal_type, signal.entity_name, signal.entity_id
        );
    }
}
