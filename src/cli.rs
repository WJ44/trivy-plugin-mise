use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::sbom::SbomMode;

/// Top-level CLI for `trivy-plugin-mise` (invoked directly or via `trivy mise`).
#[derive(Debug, Parser)]
#[command(
    name = "trivy-plugin-mise",
    version,
    about = "Bridge mise with Trivy for CycloneDX SBOM generation and vulnerability scanning",
    propagate_version = true,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub scan: ScanArgs,
}

impl Cli {
    /// Returns the resolved command, defaulting to `Commands::Scan` when no explicit subcommand was provided.
    #[must_use]
    pub fn into_command(self) -> Commands {
        self.command.unwrap_or(Commands::Scan(self.scan))
    }
}

/// Supported CLI subcommands.
#[derive(Debug, Subcommand, PartialEq, Eq)]
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
        /// Skip SHA-256 digest verification for sidecar lockfiles.
        #[arg(long)]
        no_verify_digests: bool,
        /// Include machine-local `mise` config and lockfiles (`*.local.toml`, `*.local.lock`).
        #[arg(long)]
        include_local: bool,
        /// Custom `mise` configuration file path(s) (repeatable; overrides standard config discovery).
        #[arg(long = "mise-config", value_name = "PATH")]
        mise_config: Vec<PathBuf>,
        /// Custom `mise` lockfile path(s) (repeatable; overrides standard lockfile discovery).
        #[arg(long = "mise-lock", value_name = "PATH")]
        mise_lock: Vec<PathBuf>,
        /// Target repository path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Additional arguments forwarded directly to underlying `trivy fs` invocations (after `--`).
        #[arg(last = true, value_name = "TRIVY_ARGS")]
        trivy_args: Vec<String>,
    },
    /// Scan the repository and mise-managed tool dependencies for vulnerabilities and licenses.
    Scan(ScanArgs),
    /// Print the CLI version.
    Version,
}

/// Arguments for the `scan` subcommand (also flattened onto the top-level CLI as the default command).
#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct ScanArgs {
    /// Target directory to scan.
    #[arg(default_value = ".")]
    pub target: PathBuf,

    /// Path to Trivy configuration file (defaults to `<target>/trivy.yaml` if present).
    #[arg(short = 'c', long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Path to Trivy ignore file (defaults to `<target>/.trivyignore.yaml` or `<target>/.trivyignore` if present).
    #[arg(long, value_name = "PATH")]
    pub ignorefile: Option<PathBuf>,

    /// Comma-separated list of severities to include (e.g. `HIGH,CRITICAL`).
    #[arg(short = 's', long, value_name = "SEVERITY")]
    pub severity: Option<String>,

    /// Suppress progress headers and pass `--quiet` to underlying Trivy invocations.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Ignore unfixed vulnerabilities (forwarded to Trivy).
    #[arg(long)]
    pub ignore_unfixed: bool,

    /// Comma-separated list of scanners for the repository scan tier.
    #[arg(long, default_value = "vuln,license", value_name = "SCANNERS")]
    pub scanners: String,

    /// Exit code to return when unaccepted findings are detected.
    #[arg(long, default_value_t = 1, value_name = "CODE")]
    pub exit_code: i32,

    /// Also scan active mise tool install directories via `trivy rootfs --scanners vuln`.
    #[arg(long)]
    pub include_gobinary: bool,

    /// Scan only the repository target and skip mise sidecar lock directories (and `--include-gobinary`).
    #[arg(long, conflicts_with = "toolchain_only")]
    pub repo_only: bool,

    /// Scan only mise sidecar lockfiles (plus `--include-gobinary` if requested) and skip the repository root scan.
    #[arg(long, conflicts_with = "repo_only")]
    pub toolchain_only: bool,

    /// Include machine-local `mise` config and lockfiles (`*.local.toml`, `*.local.lock`).
    #[arg(long)]
    pub include_local: bool,

    /// Custom `mise` configuration file path(s) (repeatable; overrides standard config discovery).
    #[arg(long = "mise-config", value_name = "PATH")]
    pub mise_config: Vec<PathBuf>,

    /// Custom `mise` lockfile path(s) (repeatable; overrides standard lockfile discovery).
    #[arg(long = "mise-lock", value_name = "PATH")]
    pub mise_lock: Vec<PathBuf>,

    /// Additional arguments forwarded directly to underlying `trivy` invocations (after `--`).
    #[arg(last = true, value_name = "TRIVY_ARGS")]
    pub trivy_args: Vec<String>,
}

impl Default for ScanArgs {
    fn default() -> Self {
        Self {
            target: PathBuf::from("."),
            config: None,
            ignorefile: None,
            severity: None,
            quiet: false,
            ignore_unfixed: false,
            scanners: "vuln,license".to_string(),
            exit_code: 1,
            include_gobinary: false,
            repo_only: false,
            toolchain_only: false,
            include_local: false,
            mise_config: Vec::new(),
            mise_lock: Vec::new(),
            trivy_args: Vec::new(),
        }
    }
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
            Cli::try_parse_from(["trivy-plugin-mise", "sbom"]).map(Cli::into_command),
            Ok(Commands::Sbom {
                mode: SbomMode::Formulation,
                output: None,
                platform: None,
                no_verify_digests: false,
                ref trivy_args,
                ..
            }) if trivy_args.is_empty()
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "trivy-plugin-mise",
                "sbom",
                "--mode",
                "components",
                "--output",
                "sbom-tools.cdx.json",
                ".",
                "--",
                "--skip-dirs",
                "vendor"
            ])
            .map(Cli::into_command),
            Ok(Commands::Sbom {
                mode: SbomMode::Components,
                output: Some(_),
                ref trivy_args,
                ..
            }) if trivy_args == &vec!["--skip-dirs".to_string(), "vendor".to_string()]
        ));
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "scan"]).map(Cli::into_command),
            Ok(Commands::Scan(_))
        ));
        assert!(matches!(
            Cli::try_parse_from(["trivy-plugin-mise", "version"]).map(Cli::into_command),
            Ok(Commands::Version)
        ));
    }

    #[test]
    fn parse_default_scan_without_subcommand() {
        let default_cli =
            Cli::try_parse_from(["trivy-plugin-mise"]).expect("bare invocation should parse");
        assert!(default_cli.command.is_none());
        assert_eq!(
            default_cli.into_command(),
            Commands::Scan(ScanArgs::default())
        );

        let top_level_scan = Cli::try_parse_from([
            "trivy-plugin-mise",
            "/tmp/project",
            "-c",
            "custom-trivy.yaml",
            "-s",
            "HIGH,CRITICAL",
            "-q",
            "--ignore-unfixed",
            "--scanners",
            "vuln",
            "--",
            "--skip-db-update",
            "--offline-scan",
        ])
        .expect("top-level scan args should parse");
        assert!(top_level_scan.command.is_none());

        match top_level_scan.into_command() {
            Commands::Scan(args) => {
                assert_eq!(args.target, PathBuf::from("/tmp/project"));
                assert_eq!(args.config, Some(PathBuf::from("custom-trivy.yaml")));
                assert_eq!(args.severity.as_deref(), Some("HIGH,CRITICAL"));
                assert!(args.quiet);
                assert!(args.ignore_unfixed);
                assert_eq!(args.scanners, "vuln");
                assert_eq!(
                    args.trivy_args,
                    vec!["--skip-db-update".to_string(), "--offline-scan".to_string()]
                );
            }
            other => panic!("expected default Scan command, got {other:?}"),
        }
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
            "--quiet",
            "--ignore-unfixed",
            "--scanners",
            "vuln",
            "--exit-code",
            "2",
            "--include-gobinary",
            "--",
            "--skip-files",
            "foo.lock",
        ])
        .expect("scan flags should parse");

        match cli.into_command() {
            Commands::Scan(args) => {
                assert_eq!(args.target, PathBuf::from("/tmp/project"));
                assert_eq!(args.config, Some(PathBuf::from("custom-trivy.yaml")));
                assert_eq!(args.ignorefile, Some(PathBuf::from("custom-ignore.yaml")));
                assert_eq!(args.severity.as_deref(), Some("HIGH,CRITICAL"));
                assert!(args.quiet);
                assert!(args.ignore_unfixed);
                assert_eq!(args.scanners, "vuln");
                assert_eq!(args.exit_code, 2);
                assert!(args.include_gobinary);
                assert!(!args.repo_only);
                assert!(!args.toolchain_only);
                assert_eq!(
                    args.trivy_args,
                    vec!["--skip-files".to_string(), "foo.lock".to_string()]
                );
            }
            other => panic!("expected Scan command, got {other:?}"),
        }
    }

    #[test]
    fn parse_scan_tier_flags() {
        let repo_only = Cli::try_parse_from(["trivy-plugin-mise", "scan", "--repo-only"])
            .expect("--repo-only should parse");
        match repo_only.into_command() {
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
        match toolchain_only.into_command() {
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
        match repo_with_gobinary.into_command() {
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

    #[test]
    fn parse_mise_discovery_flags_on_sbom_and_scan() {
        let sbom_cli = Cli::try_parse_from([
            "trivy-plugin-mise",
            "sbom",
            "--include-local",
            "--mise-config",
            "custom/a.toml",
            "--mise-config",
            "custom/b.toml",
            "--mise-lock",
            "custom/a.lock",
            "--mise-lock",
            "custom/b.lock",
            "/tmp/project",
        ])
        .expect("sbom mise discovery flags should parse");

        match sbom_cli.into_command() {
            Commands::Sbom {
                include_local,
                mise_config,
                mise_lock,
                path,
                ..
            } => {
                assert!(include_local);
                assert_eq!(
                    mise_config,
                    vec![
                        PathBuf::from("custom/a.toml"),
                        PathBuf::from("custom/b.toml")
                    ]
                );
                assert_eq!(
                    mise_lock,
                    vec![
                        PathBuf::from("custom/a.lock"),
                        PathBuf::from("custom/b.lock")
                    ]
                );
                assert_eq!(path, PathBuf::from("/tmp/project"));
            }
            other => panic!("expected Sbom command, got {other:?}"),
        }

        let scan_cli = Cli::try_parse_from([
            "trivy-plugin-mise",
            "scan",
            "--include-local",
            "--mise-config",
            "custom/a.toml",
            "--mise-config",
            "custom/b.toml",
            "--mise-lock",
            "custom/a.lock",
            "/tmp/project",
        ])
        .expect("scan mise discovery flags should parse");

        match scan_cli.into_command() {
            Commands::Scan(args) => {
                assert!(args.include_local);
                assert_eq!(
                    args.mise_config,
                    vec![
                        PathBuf::from("custom/a.toml"),
                        PathBuf::from("custom/b.toml")
                    ]
                );
                assert_eq!(args.mise_lock, vec![PathBuf::from("custom/a.lock")]);
                assert_eq!(args.target, PathBuf::from("/tmp/project"));
            }
            other => panic!("expected Scan command, got {other:?}"),
        }

        let default_scan_cli = Cli::try_parse_from([
            "trivy-plugin-mise",
            "--include-local",
            "--mise-config",
            "custom/a.toml",
            "--mise-lock",
            "custom/a.lock",
            "/tmp/project",
        ])
        .expect("default scan mise discovery flags should parse");

        match default_scan_cli.into_command() {
            Commands::Scan(args) => {
                assert!(args.include_local);
                assert_eq!(args.mise_config, vec![PathBuf::from("custom/a.toml")]);
                assert_eq!(args.mise_lock, vec![PathBuf::from("custom/a.lock")]);
                assert_eq!(args.target, PathBuf::from("/tmp/project"));
            }
            other => panic!("expected Scan command, got {other:?}"),
        }
    }
}
