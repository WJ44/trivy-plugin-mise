use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::sbom::SbomMode;

/// Top-level CLI for `trivy-plugin-mise` (invoked directly or via `trivy mise`).
#[derive(Debug, Parser)]
#[command(
    name = "trivy-plugin-mise",
    version,
    about = "Bridge mise with Trivy for CycloneDX SBOM generation and vulnerability scanning",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Supported CLI subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Generate a CycloneDX SBOM including mise-managed tools and dependencies.
    Sbom {
        /// Output mode (`formulation` for product SBOM + toolchain MBOM, or `components` for standalone toolchain SBOM).
        #[arg(long, value_enum, default_value_t = SbomMode::Formulation)]
        mode: SbomMode,
        /// Write CycloneDX JSON to a file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target platform triple (for example `linux-x64`) for platform-specific hashes and download URLs.
        #[arg(long)]
        platform: Option<String>,
        /// Skip SHA-256 digest verification for `.mise/locks/` sidecar lockfiles.
        #[arg(long)]
        no_verify_digests: bool,
        /// Target repository path.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Scan the repository and mise-managed tool dependencies for vulnerabilities and licenses.
    Scan,
    /// Print the CLI version.
    Version,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_subcommands() {
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "sbom"]).map(|c| c.command),
            Ok(Commands::Sbom {
                mode: SbomMode::Formulation,
                output: None,
                platform: None,
                no_verify_digests: false,
                ..
            })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "trivy-plugin-mise",
                "sbom",
                "--mode",
                "components",
                "--output",
                "sbom-tools.cdx.json",
                "."
            ])
            .map(|c| c.command),
            Ok(Commands::Sbom {
                mode: SbomMode::Components,
                output: Some(_),
                ..
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "scan"]).map(|c| c.command),
            Ok(Commands::Scan)
        ));
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "version"]).map(|c| c.command),
            Ok(Commands::Version)
        ));
    }
}
