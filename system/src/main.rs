// SPDX-License-Identifier: MIT OR Apache-2.0
//! AION-MEDSAFE: Medicaid fraud intelligence with cryptographic provenance.
//!
//! This binary orchestrates:
//! 1. Bulk data ingestion with provenance sealing (aion-context manifests)
//! 2. Policy-gated risk signal computation (verified .aion policy files)
//! 3. Trust Graph construction and query
//! 4. Sealed release export (SLSA-attested)

mod adjudication;
mod build;
mod decisions;
mod detection;
mod error;
mod graph;
mod ingest;
mod packet;
mod policy;
mod provenance;
mod resolve;
mod signals;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aion-medsafe")]
#[command(about = "Medicaid fraud intelligence with cryptographic provenance")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest a bulk data file and seal its provenance
    Ingest {
        /// Path to the raw data file (CSV, Excel, etc.)
        #[arg(short, long)]
        file: std::path::PathBuf,

        /// Source identifier (e.g., "leie_updated", "nppes_deactivated")
        #[arg(short, long)]
        source: String,

        /// Path to write the provenance manifest (.aion)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Verify provenance of a previously ingested data file
    Verify {
        /// Path to the provenance manifest (.aion)
        #[arg(short, long)]
        manifest: std::path::PathBuf,

        /// Path to the data file to verify against
        #[arg(short, long)]
        file: std::path::PathBuf,
    },

    /// Compute risk signals (requires verified policy + sealed graph)
    Signals {
        /// Path to the sealed detection policy (.aion)
        #[arg(short, long)]
        policy: std::path::PathBuf,

        /// Path to the sealed Trust Graph (.aion, payload = graph NDJSON)
        #[arg(short, long)]
        graph: std::path::PathBuf,

        /// Jurisdiction filter (e.g., "HI" for Hawaii)
        #[arg(short, long)]
        jurisdiction: Option<String>,

        /// Where to write the sealed signal output (.aion). Defaults to
        /// provenance/signals_{jurisdiction}_{date}.aion
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Seal a plaintext detection policy (YAML) into a signed .aion
    SealPolicy {
        /// Path to the plaintext policy rules (YAML)
        #[arg(short, long)]
        rules: std::path::PathBuf,

        /// Path to write the sealed policy (.aion)
        #[arg(short, long)]
        output: std::path::PathBuf,
    },

    /// Seal a Trust Graph export (NDJSON) into a signed .aion
    SealGraph {
        /// Path to the Trust Graph export (typed NDJSON)
        #[arg(short, long)]
        input: std::path::PathBuf,

        /// Path to write the sealed graph (.aion)
        #[arg(short, long)]
        output: std::path::PathBuf,
    },

    /// Build the Trust Graph from normalized NDJSON (entity resolution) and seal it
    BuildGraph {
        /// Directory holding the per-source normalized NDJSON files
        #[arg(short, long, default_value = "../pipeline/data/normalized")]
        normalized: std::path::PathBuf,

        /// Path to write the sealed Trust Graph (.aion)
        #[arg(short, long, default_value = "provenance/trust_graph.aion")]
        output: std::path::PathBuf,

        /// Sealed human review decisions to apply (confirm → merge, reject → suppress)
        #[arg(short, long, default_value = decisions::DEFAULT_DECISIONS_PATH)]
        decisions: std::path::PathBuf,
    },

    /// Enroll a reviewer (analyst/legal/admin, 80010+): generate + register a signing key
    EnrollAnalyst {
        /// Author id for the reviewer (e.g. 80010 for an analyst)
        #[arg(short, long)]
        author: u64,

        /// Path to the key registry
        #[arg(short, long, default_value = ".aion/medsafe.registry.json")]
        registry: std::path::PathBuf,
    },

    /// Record a human review decision on an identity link (confirm or reject)
    Decide {
        /// First entity id
        #[arg(short, long)]
        a: String,

        /// Second entity id
        #[arg(short, long)]
        b: String,

        /// Verdict: "confirm" (same provider) or "reject" (distinct)
        #[arg(short, long)]
        decision: String,

        /// Reviewer author id (must be enrolled via `enroll-analyst`); the
        /// decision is signed with this author's key — cryptographic attribution
        #[arg(long)]
        author: u64,

        /// Optional free-text reason
        #[arg(long, default_value = "")]
        reason: String,

        /// Path to the sealed decision log (.aion)
        #[arg(long, default_value = decisions::DEFAULT_DECISIONS_PATH)]
        decisions: std::path::PathBuf,
    },

    /// List the current human review decisions
    Decisions {
        /// Path to the sealed decision log (.aion)
        #[arg(long, default_value = decisions::DEFAULT_DECISIONS_PATH)]
        decisions: std::path::PathBuf,
    },

    /// Record a reviewer's verdict on a fired signal (true/false positive)
    Adjudicate {
        /// Signal id (from the signal output)
        #[arg(short, long)]
        signal_id: String,

        /// Signal type (e.g. re_exclusion)
        #[arg(short, long)]
        signal_type: String,

        /// Entity id the signal was raised on
        #[arg(short, long)]
        entity: String,

        /// Verdict: "tp" (true positive) or "fp" (false positive)
        #[arg(short, long)]
        verdict: String,

        /// Reviewer author id (must be enrolled via `enroll-analyst`)
        #[arg(short, long)]
        author: u64,

        /// Optional free-text reason
        #[arg(long, default_value = "")]
        reason: String,
    },

    /// Show earned per-signal-type precision from adjudicated outcomes
    Calibrate {},

    /// Generate court-defensible case packets for flagged providers
    Packet {
        /// Path to the sealed detection policy (.aion)
        #[arg(short, long)]
        policy: std::path::PathBuf,

        /// Path to the sealed Trust Graph (.aion)
        #[arg(short, long)]
        graph: std::path::PathBuf,

        /// Jurisdiction filter (e.g., "HI")
        #[arg(short, long)]
        jurisdiction: Option<String>,

        /// Generate a packet for a single entity id only
        #[arg(short, long)]
        entity: Option<String>,

        /// Where to write the sealed packets (.aion)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Directory for rendered Markdown dossiers
        #[arg(short, long, default_value = "packets")]
        render_dir: std::path::PathBuf,

        /// Cap the number of packets written (and rendered)
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show provenance chain for a data source
    Provenance {
        /// Path to the provenance manifest (.aion)
        #[arg(short, long)]
        manifest: std::path::PathBuf,
    },

    /// Initialize the key registry and signing keys
    Init {
        /// Path to store the registry
        #[arg(short, long, default_value = ".aion/medsafe.registry.json")]
        registry: std::path::PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize structured tracing (matching aion-context observability style)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("MEDSAFE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest {
            file,
            source,
            output,
        } => ingest::run(&file, &source, output.as_deref()),

        Commands::Verify { manifest, file } => provenance::verify(&manifest, &file),

        Commands::Signals {
            policy,
            graph,
            jurisdiction,
            output,
        } => signals::run(&policy, &graph, jurisdiction.as_deref(), output.as_deref()),

        Commands::SealPolicy { rules, output } => seal_artifact(&rules, &output, "policy"),

        Commands::SealGraph { input, output } => seal_artifact(&input, &output, "trust_graph"),

        Commands::BuildGraph {
            normalized,
            output,
            decisions,
        } => {
            let stats = build::run(&normalized, &output, &decisions)?;
            println!("✓ Built + sealed Trust Graph: {}", output.display());
            println!("  Records resolved: {}", stats.records);
            println!("  Entities: {}", stats.entities);
            println!(
                "    merges — npi: {}, name: {}, fuzzy auto-link: {}",
                stats.npi_merges, stats.name_merges, stats.fuzzy_auto_links
            );
            if stats.confirmed_applied > 0 || stats.rejected > 0 {
                println!(
                    "    review decisions — confirmed merges: {}, rejected (suppressed): {}",
                    stats.confirmed_applied, stats.rejected
                );
            }
            println!("  Exclusion events: {}", stats.events);
            println!("  Review link candidates: {}", stats.candidates);
            if stats.blocks_capped > 0 {
                println!("  Blocks skipped (over size cap): {}", stats.blocks_capped);
            }
            Ok(())
        }

        Commands::EnrollAnalyst { author, registry } => {
            provenance::enroll_author(&registry, author)?;
            println!("✓ Enrolled author {author}");
            println!("  Registry: {} (public key added)", registry.display());
            println!(
                "  Private key: {}",
                provenance::author_key_path(author).display()
            );
            Ok(())
        }

        Commands::Decide {
            a,
            b,
            decision,
            author,
            reason,
            decisions,
        } => decide(&a, &b, &decision, author, &reason, &decisions),

        Commands::Decisions { decisions } => list_decisions(&decisions),

        Commands::Adjudicate {
            signal_id,
            signal_type,
            entity,
            verdict,
            author,
            reason,
        } => adjudicate(&signal_id, &signal_type, &entity, &verdict, author, &reason),

        Commands::Calibrate {} => calibrate_report(),

        Commands::Packet {
            policy,
            graph,
            jurisdiction,
            entity,
            output,
            render_dir,
            limit,
        } => packet::run(
            &policy,
            &graph,
            jurisdiction.as_deref(),
            entity.as_deref(),
            output.as_deref(),
            &render_dir,
            limit,
        ),

        Commands::Provenance { manifest } => provenance::show(&manifest),

        Commands::Init { registry } => {
            provenance::init_registry(&registry)?;
            tracing::info!(event = "registry_initialized", path = %registry.display());
            Ok(())
        }
    }
}

/// Record an analyst's true/false-positive verdict on a fired signal.
fn adjudicate(
    signal_id: &str,
    signal_type: &str,
    entity: &str,
    verdict: &str,
    author: u64,
    reason: &str,
) -> anyhow::Result<()> {
    let verdict = match verdict {
        "tp" | "true_positive" => adjudication::TRUE_POSITIVE,
        "fp" | "false_positive" => adjudication::FALSE_POSITIVE,
        other => anyhow::bail!("verdict must be \"tp\" or \"fp\", got {other:?}"),
    };
    let registry =
        provenance::load_registry(std::path::Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let signing_key = provenance::load_signing_key_for(author)?;
    let record = adjudication::Adjudication {
        signal_id: signal_id.to_string(),
        signal_type: signal_type.to_string(),
        entity_id: entity.to_string(),
        verdict: verdict.to_string(),
        reviewer: author.to_string(),
        reason: reason.to_string(),
        adjudicated_at: chrono::Utc::now().to_rfc3339(),
    };
    let path = std::path::Path::new(adjudication::DEFAULT_ADJUDICATIONS_PATH);
    let total = adjudication::record(path, &registry, author, &signing_key, record)?;
    println!("✓ Adjudicated {signal_id} ({signal_type}) as {verdict} by author {author}");
    println!(
        "  Sealed adjudication log: {} ({total} verdicts)",
        path.display()
    );
    Ok(())
}

/// Print earned per-signal-type precision from the adjudication log.
fn calibrate_report() -> anyhow::Result<()> {
    let registry =
        provenance::load_registry(std::path::Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let path = std::path::Path::new(adjudication::DEFAULT_ADJUDICATIONS_PATH);
    let cal = adjudication::calibrate(&adjudication::load(path, &registry)?);
    if cal.is_empty() {
        println!("No adjudications recorded yet ({}).", path.display());
        return Ok(());
    }
    println!("Calibration — earned precision per signal type:");
    for (kind, stats) in &cal {
        let precision = stats
            .precision()
            .map(|p| format!("{:.0}%", p * 100.0))
            .unwrap_or_else(|| "—".to_string());
        println!(
            "  {kind:<34}  n={:<4} tp={:<4} fp={:<4} precision={precision}",
            stats.total(),
            stats.tp,
            stats.fp
        );
    }
    Ok(())
}

/// Record a human review decision (confirm/reject) on an identity link.
fn decide(
    a: &str,
    b: &str,
    decision: &str,
    author: u64,
    reason: &str,
    decisions_path: &std::path::Path,
) -> anyhow::Result<()> {
    if decision != decisions::CONFIRM && decision != decisions::REJECT {
        anyhow::bail!(
            "decision must be \"{}\" or \"{}\", got {decision:?}",
            decisions::CONFIRM,
            decisions::REJECT
        );
    }
    let registry =
        provenance::load_registry(std::path::Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    // Sign with the analyst's enrolled key — fails if the author isn't enrolled.
    let signing_key = provenance::load_signing_key_for(author)?;
    let record = decisions::Decision {
        entity_a: a.to_string(),
        entity_b: b.to_string(),
        decision: decision.to_string(),
        reviewer: author.to_string(),
        reason: reason.to_string(),
        decided_at: chrono::Utc::now().to_rfc3339(),
    };
    let total = decisions::record(decisions_path, &registry, author, &signing_key, record)?;

    println!("✓ Recorded {decision}: {a} ⇄ {b} (signed by author {author})");
    println!(
        "  Sealed decision log: {} ({total} decisions)",
        decisions_path.display()
    );
    println!("  Apply with: aion-medsafe build-graph");
    Ok(())
}

/// List the current (latest-per-pair) human review decisions.
fn list_decisions(decisions_path: &std::path::Path) -> anyhow::Result<()> {
    let registry =
        provenance::load_registry(std::path::Path::new(provenance::DEFAULT_REGISTRY_PATH))?;
    let all = decisions::load(decisions_path, &registry)?;
    if all.is_empty() {
        println!("No decisions recorded ({})", decisions_path.display());
        return Ok(());
    }
    let verdicts = decisions::verdicts(&all);
    println!(
        "Decisions in {} — {} confirmed, {} rejected:",
        decisions_path.display(),
        verdicts.confirmed.len(),
        verdicts.rejected.len()
    );
    for d in &all {
        println!(
            "  [{}] {} ⇄ {}  by {} ({})",
            d.decision, d.entity_a, d.entity_b, d.reviewer, d.decided_at
        );
    }
    Ok(())
}

/// Seal a plaintext governance artifact (policy YAML, graph NDJSON) into a
/// signed `.aion` whose payload IS the data. Shared by `seal-policy` and
/// `seal-graph`.
fn seal_artifact(
    input: &std::path::Path,
    output: &std::path::Path,
    kind: &str,
) -> anyhow::Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file not found: {}", input.display());
    }
    let payload = std::fs::read(input)?;
    let signing_key = provenance::load_signing_key()?;
    let sealed = provenance::seal_payload(
        output,
        &payload,
        &signing_key,
        &format!("Seal {kind}: {}", input.display()),
    )?;

    println!("✓ Sealed {kind}: {}", input.display());
    println!("  Size: {} bytes", payload.len());
    println!("  Sealed: {}", output.display());
    println!("  Payload hash: {}", hex::encode(sealed));
    Ok(())
}
