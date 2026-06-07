// SPDX-License-Identifier: MIT OR Apache-2.0
//! AION-MEDSAFE: Medicaid fraud intelligence with cryptographic provenance.
//!
//! This binary orchestrates:
//! 1. Bulk data ingestion with provenance sealing (aion-context manifests)
//! 2. Policy-gated risk signal computation (verified .aion policy files)
//! 3. Trust Graph construction and query
//! 4. Sealed release export (SLSA-attested)

// In-progress: several error variants, policy fields, and policy accessors are
// part of the public API surface but not yet wired into the binary's command
// paths. Allowed crate-wide so the pre-commit `-D warnings` gate still catches
// every *other* warning. Remove once these are consumed.
#![allow(dead_code)]

mod error;
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

    /// Compute risk signals (requires verified policy)
    Signals {
        /// Path to the detection policy (.aion)
        #[arg(short, long)]
        policy: std::path::PathBuf,

        /// Path to the Trust Graph export (NDJSON)
        #[arg(short, long)]
        graph: std::path::PathBuf,

        /// Jurisdiction filter (e.g., "HI" for Hawaii)
        #[arg(short, long)]
        jurisdiction: Option<String>,
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
        } => signals::run(&policy, &graph, jurisdiction.as_deref()),

        Commands::Provenance { manifest } => provenance::show(&manifest),

        Commands::Init { registry } => {
            provenance::init_registry(&registry)?;
            tracing::info!(event = "registry_initialized", path = %registry.display());
            Ok(())
        }
    }
}
