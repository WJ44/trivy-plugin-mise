use std::process::ExitCode;

use clap::Parser;
use trivy_plugin_mise::cli::{Cli, Commands};
use trivy_plugin_mise::sbom::{SbomOptions, generate_sbom, write_sbom};
use trivy_plugin_mise::scan::{ScanOptions, run_scan};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.into_command() {
        Commands::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Commands::Sbom {
            mode,
            output,
            platform,
            no_verify_digests,
            include_local,
            mise_config,
            mise_lock,
            path,
            trivy_args,
        } => {
            let options = SbomOptions {
                mode,
                target_platform: platform,
                verify_digests: !no_verify_digests,
                include_local,
                mise_configs: mise_config,
                mise_locks: mise_lock,
                trivy_args,
            };
            match generate_sbom(&path, &options).and_then(|bom| write_sbom(&bom, output.as_deref()))
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            }
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
