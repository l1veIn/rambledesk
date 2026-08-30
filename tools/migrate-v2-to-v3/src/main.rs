use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rambledesk_migrate_v2_to_v3::inspect;

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { source_db } => {
            let report = inspect(&source_db).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}
