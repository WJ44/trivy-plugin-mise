use std::process::ExitCode;

use clap::Parser;
use trivy_plugin_mise::cli::{Cli, Commands};
use trivy_plugin_mise::scan::{ScanOptions, run_scan};

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
        Commands::Scan(args) => {
            let options = ScanOptions::from(args);
            match run_scan(&options) {
                Ok(outcome) if outcome.findings_detected => {
                    let code = u8::try_from(outcome.exit_code).unwrap_or(1);
                    ExitCode::from(code)
                }
                Ok(_) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
