use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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

    /// Scan only the repository target and skip `.mise/locks/` (and `--include-gobinary`).
    #[arg(long, conflicts_with = "toolchain_only")]
    pub repo_only: bool,

    /// Scan only `.mise/locks/` sidecar lockfiles (plus `--include-gobinary` if requested) and skip the repository root scan.
    #[arg(long, conflicts_with = "repo_only")]
    pub toolchain_only: bool,
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
                assert!(!args.repo_only);
                assert!(!args.toolchain_only);
            }
            other => panic!("expected Scan command, got {other:?}"),
        }
    }

    #[test]
    fn parse_scan_tier_flags() {
        let repo_only = Cli::try_parse_from(["trivy-plugin-mise", "scan", "--repo-only"])
            .expect("--repo-only should parse");
        match repo_only.command {
            Commands::Scan(args) => {
                assert!(args.repo_only);
                assert!(!args.toolchain_only);
            }
            other => panic!("expected Scan command, got {other:?}"),
        }

        let toolchain_only = Cli::try_parse_from([
            "trivy-plugin-mise",
            "scan",
            "--toolchain-only",
            "--include-gobinary",
        ])
        .expect("--toolchain-only --include-gobinary should parse");
        match toolchain_only.command {
            Commands::Scan(args) => {
                assert!(!args.repo_only);
                assert!(args.toolchain_only);
                assert!(args.include_gobinary);
            }
            other => panic!("expected Scan command, got {other:?}"),
        }

        let repo_with_gobinary = Cli::try_parse_from([
            "trivy-plugin-mise",
            "scan",
            "--repo-only",
            "--include-gobinary",
        ])
        .expect("--repo-only --include-gobinary should parse");
        match repo_with_gobinary.command {
            Commands::Scan(args) => {
                assert!(args.repo_only);
                assert!(args.include_gobinary);
            }
            other => panic!("expected Scan command, got {other:?}"),
        }

        let conflict = Cli::try_parse_from([
            "trivy-plugin-mise",
            "scan",
            "--repo-only",
            "--toolchain-only",
        ])
        .expect_err("--repo-only and --toolchain-only must conflict");
        assert_eq!(conflict.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}
