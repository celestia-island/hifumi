use anyhow::Result;
use clap::{Parser, Subcommand};

mod analyze;
mod generate;

use analyze::analyze_struct_changes;
use generate::generate_migration_code;

/// hifumi CLI - Generate migration code from git history
#[derive(Parser)]
#[command(name = "hifumi")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze struct changes between two git commits
    Analyze {
        /// Path to the Rust source file containing the struct
        #[arg(short, long)]
        file: String,

        /// Name of the struct to analyze
        #[arg(short, long)]
        struct_name: String,

        /// From commit (older)
        #[arg(long, default_value = "HEAD~1")]
        from: String,

        /// To commit (newer)
        #[arg(long, default_value = "HEAD")]
        to: String,
    },

    /// Generate migration code from git history
    Generate {
        /// Path to the Rust source file containing the struct
        #[arg(short, long)]
        file: String,

        /// Name of the struct to analyze
        #[arg(short, long)]
        struct_name: String,

        /// From version string
        #[arg(long)]
        from_version: String,

        /// To version string
        #[arg(long)]
        to_version: String,

        /// From commit (older)
        #[arg(long, default_value = "HEAD~1")]
        from_commit: String,

        /// To commit (newer)
        #[arg(long, default_value = "HEAD")]
        to_commit: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            file,
            struct_name,
            from,
            to,
        } => {
            let changes = analyze_struct_changes(&file, &struct_name, &from, &to)?;
            println!("{}", changes);
        }
        Commands::Generate {
            file,
            struct_name,
            from_version,
            to_version,
            from_commit,
            to_commit,
        } => {
            let code = generate_migration_code(
                &file,
                &struct_name,
                &from_version,
                &to_version,
                &from_commit,
                &to_commit,
            )?;
            println!("{}", code);
        }
    }

    Ok(())
}
