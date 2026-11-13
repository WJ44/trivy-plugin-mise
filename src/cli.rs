use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    Scan(ScanArgs),
    /// Print the CLI version.
    Version,
}

/// Arguments for the `scan` subcommand.
#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct ScanArgs {
    /// Target directory to scan.
    #[arg(default_value = ".")]
    pub target: PathBuf,

    /// Path to Trivy configuration file (defaults to `<target>/trivy.yaml` if present).
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Path to Trivy ignore file (defaults to `<target>/.trivyignore.yaml` if present).
    #[arg(long, value_name = "PATH")]
    pub ignorefile: Option<PathBuf>,

    /// Comma-separated list of severities to include (e.g. `HIGH,CRITICAL`).
    #[arg(long, value_name = "SEVERITY")]
    pub severity: Option<String>,

    /// Exit code to return when unaccepted findings are detected.
    #[arg(long, default_value_t = 1, value_name = "CODE")]
    pub exit_code: i32,

    /// Also scan active mise tool install directories via `trivy rootfs --scanners vuln`.
    #[arg(long)]
    pub include_gobinary: bool,
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
            Ok(Commands::Scan(_))
        ));
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "version"]).map(|c| c.command),
            Ok(Commands::Version)
        ));
    }

    #[test]
    fn parse_scan_flags() {
        let cli = Cli::try_parse_from([
            "trivy-plugin-mise",
            "scan",
            "/tmp/project",
            "--config",
            "custom-trivy.yaml",
            "--ignorefile",
            "custom-ignore.yaml",
            "--severity",
            "HIGH,CRITICAL",
            "--exit-code",
            "2",
            "--include-gobinary",
        ])
        .expect("scan flags should parse");

        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.target, PathBuf::from("/tmp/project"));
                assert_eq!(args.config, Some(PathBuf::from("custom-trivy.yaml")));
                assert_eq!(args.ignorefile, Some(PathBuf::from("custom-ignore.yaml")));
                assert_eq!(args.severity.as_deref(), Some("HIGH,CRITICAL"));
                assert_eq!(args.exit_code, 2);
                assert!(args.include_gobinary);
            }
            other => panic!("expected Scan command, got {other:?}"),
        }
    }
}
