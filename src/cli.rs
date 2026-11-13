use clap::{Parser, Subcommand};

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
    Sbom,
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
            Ok(Commands::Sbom)
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
