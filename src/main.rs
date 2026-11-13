use std::process::ExitCode;

use clap::Parser;
use trivy_plugin_mise::cli::{Cli, Commands};
use trivy_plugin_mise::sbom::{SbomOptions, generate_sbom, write_sbom};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Commands::Sbom {
            mode,
            output,
            platform,
            no_verify_digests,
            path,
        } => {
            let options = SbomOptions {
                mode,
                target_platform: platform,
                verify_digests: !no_verify_digests,
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
        Commands::Scan => {
            eprintln!("error: `scan` is not yet implemented");
            ExitCode::FAILURE
        }
    }
}
