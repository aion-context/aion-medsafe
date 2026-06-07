// SPDX-License-Identifier: MIT OR Apache-2.0
//! AION-MEDSAFE: Medicaid fraud intelligence with cryptographic provenance.
//!
//! This binary orchestrates:
//! 1. Bulk data ingestion with provenance sealing (aion-context manifests)
//! 2. Policy-gated risk signal computation (verified .aion policy files)
//! 3. Trust Graph construction and query
//! 4. Sealed release export (SLSA-attested)

mod detection;
mod error;
mod graph;
mod ingest;
mod policy;
mod provenance;
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

        Commands::Provenance { manifest } => provenance::show(&manifest),

        Commands::Init { registry } => {
            provenance::init_registry(&registry)?;
            tracing::info!(event = "registry_initialized", path = %registry.display());
            Ok(())
        }
    }
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
