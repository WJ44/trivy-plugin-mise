use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use thiserror::Error;

use crate::cli::ScanArgs;
use crate::mise_lock::{MiseLock, MiseLockError};
use crate::sidecar::{SidecarError, SidecarOptions, materialize_sidecars};

/// Errors returned while running `trivy-plugin-mise scan`.
#[derive(Debug, Error)]
pub enum ScanError {
    #[error("scan target `{path}` does not exist")]
    InvalidTarget { path: String },

    #[error("cannot combine `--repo-only` and `--toolchain-only`")]
    ConflictingTiers,

    #[error(transparent)]
    MiseLock(#[from] MiseLockError),

    #[error(transparent)]
    Sidecar(#[from] SidecarError),

    #[error("failed to invoke `trivy`: {0}")]
    TrivyInvocation(#[source] std::io::Error),

    #[error("`{command}` failed with status {status}")]
    TrivyFailed { command: String, status: String },

    #[error("failed to invoke `mise`: {0}")]
    MiseInvocation(#[source] std::io::Error),

    #[error("`mise ls --json` failed with status {status}: {stderr}")]
    MiseFailed { status: String, stderr: String },

    #[error("failed to parse `mise ls --json` output: {0}")]
    MiseJson(#[from] serde_json::Error),

    #[error("filesystem error at `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Configuration options for the `scan` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    pub target: PathBuf,
    pub config: Option<PathBuf>,
    pub ignorefile: Option<PathBuf>,
    pub severity: Option<String>,
    pub exit_code: i32,
    pub include_gobinary: bool,
    pub repo_only: bool,
    pub toolchain_only: bool,
}

impl From<ScanArgs> for ScanOptions {
    fn from(args: ScanArgs) -> Self {
        Self {
            target: args.target,
            config: args.config,
            ignorefile: args.ignorefile,
            severity: args.severity,
            exit_code: args.exit_code,
            include_gobinary: args.include_gobinary,
            repo_only: args.repo_only,
            toolchain_only: args.toolchain_only,
        }
    }
}

impl From<&ScanArgs> for ScanOptions {
    fn from(args: &ScanArgs) -> Self {
        Self {
            target: args.target.clone(),
            config: args.config.clone(),
            ignorefile: args.ignorefile.clone(),
            severity: args.severity.clone(),
            exit_code: args.exit_code,
            include_gobinary: args.include_gobinary,
            repo_only: args.repo_only,
            toolchain_only: args.toolchain_only,
        }
    }
}

/// Summary of a completed `scan` execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    pub findings_detected: bool,
    pub exit_code: i32,
    pub repo_findings: bool,
    pub toolchain_findings: bool,
    pub gobinary_findings: bool,
    pub scanned_gobinary_paths: usize,
}

struct TrivyRunContext {
    config: Option<PathBuf>,
    ignorefile: Option<PathBuf>,
    severity: Option<String>,
    exit_code: i32,
}

#[derive(Debug, Deserialize)]
struct MiseToolEntry {
    #[serde(default)]
    install_path: Option<PathBuf>,
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    active: bool,
}

/// Runs repository, `.mise/locks/` toolchain, and optional binary scans via Trivy.
pub fn run_scan(options: &ScanOptions) -> Result<ScanOutcome, ScanError> {
    if options.repo_only && options.toolchain_only {
        return Err(ScanError::ConflictingTiers);
    }

    if !options.target.exists() {
        return Err(ScanError::InvalidTarget {
            path: options.target.display().to_string(),
        });
    }

    let ctx = TrivyRunContext {
        config: options
            .config
            .clone()
            .or_else(|| existing_file(options.target.join("trivy.yaml"))),
        ignorefile: options
            .ignorefile
            .clone()
            .or_else(|| existing_file(options.target.join(".trivyignore.yaml"))),
        severity: options.severity.clone(),
        exit_code: options.exit_code,
    };

    let run_repo = !options.toolchain_only;
    let run_toolchain = !options.repo_only;
    let run_gobinary = !options.repo_only && options.include_gobinary;

    let repo_findings = if run_repo {
        print_tier_line(&format!(
            "Repository scan (trivy fs --scanners vuln,license): {}",
            options.target.display()
        ))?;
        run_trivy_scan(
            "fs",
            "vuln,license",
            &options.target,
            &[".mise/locks"],
            &ctx,
        )?
    } else {
        false
    };

    let (toolchain_findings, has_toolchain_files) = if run_toolchain {
        let mise_lock_path = options.target.join("mise.lock");
        let lock = if mise_lock_path.is_file() {
            Some(MiseLock::from_path(&mise_lock_path)?)
        } else {
            None
        };

        let workspace =
            materialize_sidecars(&options.target, lock.as_ref(), &SidecarOptions::default())?;
        let has_files = workspace
            .entries()
            .iter()
            .any(|entry| !entry.materialized_files.is_empty());

        let findings = if has_files {
            print_tier_line("Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")?;
            run_trivy_scan("fs", "vuln", workspace.path(), &[], &ctx)?
        } else {
            false
        };

        workspace.close().map_err(|source| ScanError::Io {
            path: std::env::temp_dir().display().to_string(),
            source,
        })?;

        (findings, has_files)
    } else {
        (false, false)
    };

    let mut gobinary_findings = false;
    let mut scanned_gobinary_paths = 0;

    if run_gobinary {
        let install_paths = discover_active_mise_install_paths(&options.target)?;
        scanned_gobinary_paths = install_paths.len();
        for install_path in install_paths {
            print_tier_line(&format!(
                "Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln): {}",
                install_path.display()
            ))?;
            if run_trivy_scan("rootfs", "vuln", &install_path, &[], &ctx)? {
                gobinary_findings = true;
            }
        }
    }

    let findings_detected = repo_findings || toolchain_findings || gobinary_findings;
    let exit_code = if findings_detected {
        options.exit_code
    } else {
        0
    };

    let repo_status = tier_status(run_repo, true, repo_findings);
    let toolchain_status = tier_status(run_toolchain, has_toolchain_files, toolchain_findings);
    let gobinary_status =
        format_gobinary_status(run_gobinary, scanned_gobinary_paths, gobinary_findings);
    let overall_status = if findings_detected {
        format!("findings detected (exit code {exit_code})")
    } else {
        "clean (exit code 0)".to_string()
    };

    print_tier_line(&format!(
        "Scan summary: repo={repo_status}, toolchain={toolchain_status}, gobinary={gobinary_status} -> {overall_status}"
    ))?;

    Ok(ScanOutcome {
        findings_detected,
        exit_code,
        repo_findings,
        toolchain_findings,
        gobinary_findings,
        scanned_gobinary_paths,
    })
}

/// Parses `mise ls --json` output into sorted, deduplicated active install directories.
pub fn parse_active_mise_install_paths(json_bytes: &[u8]) -> Result<Vec<PathBuf>, ScanError> {
    let tools: BTreeMap<String, Vec<MiseToolEntry>> = serde_json::from_slice(json_bytes)?;
    let mut paths = BTreeSet::new();

    for entries in tools.values() {
        for entry in entries {
            if entry.installed
                && entry.active
                && let Some(install_path) = &entry.install_path
                && install_path.exists()
            {
                paths.insert(install_path.clone());
            }
        }
    }

    Ok(paths.into_iter().collect())
}

fn discover_active_mise_install_paths(target: &Path) -> Result<Vec<PathBuf>, ScanError> {
    let output = Command::new("mise")
        .args(["ls", "--json"])
        .current_dir(target)
        .output()
        .map_err(ScanError::MiseInvocation)?;

    if !output.status.success() {
        return Err(ScanError::MiseFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    parse_active_mise_install_paths(&output.stdout)
}

fn run_trivy_scan(
    subcommand: &str,
    scanners: &str,
    target: &Path,
    skip_dirs: &[&str],
    ctx: &TrivyRunContext,
) -> Result<bool, ScanError> {
    let exit_code_str = ctx.exit_code.to_string();
    let mut cmd = Command::new("trivy");
    cmd.args([
        subcommand,
        "--scanners",
        scanners,
        "--exit-code",
        &exit_code_str,
    ]);

    for dir in skip_dirs {
        cmd.args(["--skip-dirs", dir]);
    }

    if let Some(config) = &ctx.config {
        cmd.arg("--config").arg(config);
    }
    if let Some(ignorefile) = &ctx.ignorefile {
        cmd.arg("--ignorefile").arg(ignorefile);
    }
    if let Some(severity) = &ctx.severity {
        cmd.arg("--severity").arg(severity);
    }

    cmd.arg(target);

    let status = cmd.status().map_err(ScanError::TrivyInvocation)?;
    if status.success() {
        return Ok(false);
    }

    if status.code() == Some(ctx.exit_code) {
        return Ok(true);
    }

    Err(ScanError::TrivyFailed {
        command: format!("trivy {subcommand}"),
        status: status.to_string(),
    })
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

fn print_tier_line(message: &str) -> Result<(), ScanError> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "==> {message}").map_err(|source| ScanError::Io {
        path: "<stdout>".to_string(),
        source,
    })?;
    stdout.flush().map_err(|source| ScanError::Io {
        path: "<stdout>".to_string(),
        source,
    })
}

fn tier_status(enabled: bool, had_targets: bool, findings: bool) -> &'static str {
    if !enabled {
        "skipped"
    } else if !had_targets {
        "none"
    } else if findings {
        "findings"
    } else {
        "clean"
    }
}

fn format_gobinary_status(enabled: bool, scanned_paths: usize, findings: bool) -> String {
    if !enabled {
        return "skipped".to_string();
    }
    if scanned_paths == 0 {
        return "none".to_string();
    }
    let state = if findings { "findings" } else { "clean" };
    let noun = if scanned_paths == 1 { "path" } else { "paths" };
    format!("{state} ({scanned_paths} {noun})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_active_installed_mise_paths_only() {
        let active_dir = TempDir::new().unwrap();
        let inactive_dir = TempDir::new().unwrap();
        let missing_dir = active_dir.path().join("does-not-exist");

        let payload = serde_json::json!({
            "lefthook": [
                {
                    "install_path": active_dir.path(),
                    "installed": true,
                    "active": true
                }
            ],
            "shfmt": [
                {
                    "install_path": inactive_dir.path(),
                    "installed": true,
                    "active": false
                },
                {
                    "install_path": active_dir.path(),
                    "installed": true,
                    "active": true
                }
            ],
            "uninstalled": [
                {
                    "install_path": missing_dir,
                    "installed": false,
                    "active": true
                }
            ]
        });

        let paths = parse_active_mise_install_paths(payload.to_string().as_bytes()).unwrap();
        assert_eq!(paths, vec![active_dir.path().to_path_buf()]);
    }

    #[test]
    fn rejects_conflicting_tier_flags() {
        let tmp = TempDir::new().unwrap();
        let options = ScanOptions {
            target: tmp.path().to_path_buf(),
            config: None,
            ignorefile: None,
            severity: None,
            exit_code: 1,
            include_gobinary: false,
            repo_only: true,
            toolchain_only: true,
        };

        assert!(matches!(
            run_scan(&options),
            Err(ScanError::ConflictingTiers)
        ));
    }
}
