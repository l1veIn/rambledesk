use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};
use rambledesk_migrate_v2_to_v3::{dry_run, execute, inspect, verify};

#[derive(Debug, Parser)]
#[command(name = "rambledesk-migrate-v2-to-v3")]
#[command(about = "Explicit RambleDesk v2 to v3 migration tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Classify legacy records and expected losses without writing anything.
    Inspect {
        /// Path to the legacy v2 SQLite database.
        #[arg(long)]
        source_db: PathBuf,
    },
    /// Validate or execute an explicit migration into one atomic target root.
    #[command(group(
        ArgGroup::new("migration_mode")
            .required(true)
            .multiple(false)
            .args(["dry_run", "execute"])
    ))]
    Migrate {
        /// Fully materialize and verify a private temporary target, then discard it.
        #[arg(long)]
        dry_run: bool,
        /// Materialize, verify, and atomically publish a new target root.
        #[arg(long)]
        execute: bool,
        /// Path to the legacy v2 SQLite database.
        #[arg(long)]
        source_db: PathBuf,
        /// New root containing the v3 database, Artifact Store, backup, and reports.
        #[arg(long)]
        target_root: PathBuf,
    },
    /// Verify an already published migration root without changing it.
    Verify {
        /// Published migration root.
        #[arg(long)]
        target_root: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { source_db } => {
            let report = inspect(&source_db).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Migrate {
            dry_run: is_dry_run,
            execute: execute_mode,
            source_db,
            target_root,
        } => {
            let report = if is_dry_run {
                dry_run(&source_db, &target_root).await?
            } else if execute_mode {
                execute(&source_db, &target_root).await?
            } else {
                unreachable!("clap requires exactly one migration mode")
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Verify { target_root } => {
            let report = verify(&target_root).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.valid {
                std::process::exit(2);
            }
        }
    }
    Ok(())
}
