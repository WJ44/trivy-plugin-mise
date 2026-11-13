use std::process::ExitCode;

use clap::Parser;
use trivy_plugin_mise::cli::{Cli, Commands};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Commands::Sbom => {
            eprintln!("error: `sbom` is not yet implemented");
            ExitCode::FAILURE
        }
        Commands::Scan => {
            eprintln!("error: `scan` is not yet implemented");
            ExitCode::FAILURE
        }
    }
}
