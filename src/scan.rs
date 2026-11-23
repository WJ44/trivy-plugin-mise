use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
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
    pub quiet: bool,
    pub ignore_unfixed: bool,
    pub scanners: String,
    pub exit_code: i32,
    pub include_gobinary: bool,
    pub repo_only: bool,
    pub toolchain_only: bool,
    pub trivy_args: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self::from(ScanArgs::default())
    }
}

impl From<ScanArgs> for ScanOptions {
    fn from(args: ScanArgs) -> Self {
        Self {
            target: args.target,
            config: args.config,
            ignorefile: args.ignorefile,
            severity: args.severity,
            quiet: args.quiet,
            ignore_unfixed: args.ignore_unfixed,
            scanners: args.scanners,
            exit_code: args.exit_code,
            include_gobinary: args.include_gobinary,
            repo_only: args.repo_only,
            toolchain_only: args.toolchain_only,
            trivy_args: args.trivy_args,
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
            quiet: args.quiet,
            ignore_unfixed: args.ignore_unfixed,
            scanners: args.scanners.clone(),
            exit_code: args.exit_code,
            include_gobinary: args.include_gobinary,
            repo_only: args.repo_only,
            toolchain_only: args.toolchain_only,
            trivy_args: args.trivy_args.clone(),
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
    quiet: bool,
    ignore_unfixed: bool,
    exit_code: i32,
    trivy_args: Vec<String>,
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
        ignorefile: resolve_ignorefile(&options.target, options.ignorefile.clone()),
        severity: options.severity.clone(),
        quiet: options.quiet,
        ignore_unfixed: options.ignore_unfixed,
        exit_code: options.exit_code,
        trivy_args: options.trivy_args.clone(),
    };

    let run_repo = !options.toolchain_only;
    let run_toolchain = !options.repo_only;
    let run_gobinary = !options.repo_only && options.include_gobinary;
    let repo_scanners = if options.scanners.trim().is_empty() {
        "vuln,license"
    } else {
        options.scanners.trim()
    };

    let repo_findings = if run_repo {
        print_tier_line(
            options.quiet,
            &format!(
                "Repository scan (trivy fs --scanners {repo_scanners}): {}",
                options.target.display()
            ),
        )?;
        run_trivy_scan("fs", repo_scanners, &options.target, &[".mise/locks"], &ctx)?
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
            print_tier_line(
                options.quiet,
                "Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)",
            )?;
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
            print_tier_line(
                options.quiet,
                &format!(
                    "Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln): {}",
                    install_path.display()
                ),
            )?;
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

    print_tier_line(
        options.quiet,
        &format!(
            "Scan summary: repo={repo_status}, toolchain={toolchain_status}, gobinary={gobinary_status} -> {overall_status}"
        ),
    )?;

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

fn build_trivy_scan_args(
    subcommand: &str,
    scanners: &str,
    target: &Path,
    skip_dirs: &[&str],
    ctx: &TrivyRunContext,
) -> Vec<OsString> {
    let exit_code_str = ctx.exit_code.to_string();
    let mut args = vec![
        OsString::from(subcommand),
        OsString::from("--scanners"),
        OsString::from(scanners),
        OsString::from("--exit-code"),
        OsString::from(exit_code_str),
    ];

    for dir in skip_dirs {
        args.push(OsString::from("--skip-dirs"));
        args.push(OsString::from(dir));
    }

    if let Some(config) = &ctx.config {
        args.push(OsString::from("--config"));
        args.push(config.as_os_str().to_owned());
    }
    if let Some(ignorefile) = &ctx.ignorefile {
        args.push(OsString::from("--ignorefile"));
        args.push(ignorefile.as_os_str().to_owned());
    }
    if let Some(severity) = &ctx.severity {
        args.push(OsString::from("--severity"));
        args.push(OsString::from(severity));
    }
    if ctx.quiet {
        args.push(OsString::from("--quiet"));
    }
    if ctx.ignore_unfixed {
        args.push(OsString::from("--ignore-unfixed"));
    }
    for extra in &ctx.trivy_args {
        args.push(OsString::from(extra));
    }

    args.push(target.as_os_str().to_owned());
    args
}

fn run_trivy_scan(
    subcommand: &str,
    scanners: &str,
    target: &Path,
    skip_dirs: &[&str],
    ctx: &TrivyRunContext,
) -> Result<bool, ScanError> {
    let mut cmd = Command::new("trivy");
    cmd.args(build_trivy_scan_args(
        subcommand, scanners, target, skip_dirs, ctx,
    ));

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

fn resolve_ignorefile(target: &Path, explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit
        .or_else(|| existing_file(target.join(".trivyignore.yaml")))
        .or_else(|| existing_file(target.join(".trivyignore")))
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

fn write_tier_line<W: Write>(writer: &mut W, quiet: bool, message: &str) -> Result<(), ScanError> {
    if quiet {
        return Ok(());
    }
    writeln!(writer, "==> {message}").map_err(|source| ScanError::Io {
        path: "<stderr>".to_string(),
        source,
    })?;
    writer.flush().map_err(|source| ScanError::Io {
        path: "<stderr>".to_string(),
        source,
    })
}

fn print_tier_line(quiet: bool, message: &str) -> Result<(), ScanError> {
    if quiet {
        return Ok(());
    }
    let mut stderr = std::io::stderr().lock();
    write_tier_line(&mut stderr, quiet, message)
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
    use std::fs;
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
            repo_only: true,
            toolchain_only: true,
            ..ScanOptions::default()
        };

        assert!(matches!(
            run_scan(&options),
            Err(ScanError::ConflictingTiers)
        ));
    }

    #[test]
    fn resolve_ignorefile_prefers_explicit_then_yaml_then_plain_trivyignore() {
        let tmp = TempDir::new().unwrap();

        assert_eq!(resolve_ignorefile(tmp.path(), None), None);

        // Directory named .trivyignore should not be treated as an ignorefile.
        fs::create_dir(tmp.path().join(".trivyignore")).unwrap();
        assert_eq!(resolve_ignorefile(tmp.path(), None), None);
        fs::remove_dir(tmp.path().join(".trivyignore")).unwrap();

        // Fallback to .trivyignore when .trivyignore.yaml is absent.
        let plain = tmp.path().join(".trivyignore");
        fs::write(&plain, "CVE-2020-28500\n").unwrap();
        assert_eq!(resolve_ignorefile(tmp.path(), None), Some(plain.clone()));

        // Prefer .trivyignore.yaml when both exist.
        let yaml = tmp.path().join(".trivyignore.yaml");
        fs::write(&yaml, "vulnerabilities: []\n").unwrap();
        assert_eq!(resolve_ignorefile(tmp.path(), None), Some(yaml));

        // Explicit --ignorefile always takes precedence.
        let explicit = tmp.path().join("custom.trivyignore");
        assert_eq!(
            resolve_ignorefile(tmp.path(), Some(explicit.clone())),
            Some(explicit)
        );
    }

    #[test]
    fn build_trivy_scan_args_includes_quiet_ignore_unfixed_and_trailing_args() {
        let ctx = TrivyRunContext {
            config: Some(PathBuf::from("trivy.yaml")),
            ignorefile: Some(PathBuf::from(".trivyignore")),
            severity: Some("HIGH,CRITICAL".to_string()),
            quiet: true,
            ignore_unfixed: true,
            exit_code: 2,
            trivy_args: vec!["--skip-db-update".to_string(), "--offline-scan".to_string()],
        };

        let args = build_trivy_scan_args(
            "fs",
            "vuln",
            Path::new("/tmp/project"),
            &[".mise/locks"],
            &ctx,
        );
        let strings: Vec<String> = args
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            strings,
            vec![
                "fs",
                "--scanners",
                "vuln",
                "--exit-code",
                "2",
                "--skip-dirs",
                ".mise/locks",
                "--config",
                "trivy.yaml",
                "--ignorefile",
                ".trivyignore",
                "--severity",
                "HIGH,CRITICAL",
                "--quiet",
                "--ignore-unfixed",
                "--skip-db-update",
                "--offline-scan",
                "/tmp/project",
            ]
        );
    }

    #[test]
    fn write_tier_line_emits_when_not_quiet_and_suppresses_when_quiet() {
        let mut buf = Vec::new();
        write_tier_line(&mut buf, false, "Repository scan").unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "==> Repository scan\n");

        let mut quiet_buf = Vec::new();
        write_tier_line(&mut quiet_buf, true, "Repository scan").unwrap();
        assert!(quiet_buf.is_empty());
    }
}
