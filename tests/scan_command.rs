use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use tempfile::TempDir;

// Serialize Trivy-invoking tests so concurrent threads do not race to download
// or lock the shared Trivy vulnerability database on fresh CI runners.
static TRIVY_LOCK: Mutex<()> = Mutex::new(());

fn lock_trivy() -> MutexGuard<'static, ()> {
    TRIVY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trivy-plugin-mise"))
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

const VULN_PACKAGE_LOCK: &str = r#"{
  "name": "vuln-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "node_modules/lodash": {
      "version": "4.17.20",
      "resolved": "https://registry.npmjs.org/lodash/-/lodash-4.17.20.tgz",
      "integrity": "sha512-PlhdFcillOINfeV7Ni6oF1TAEayyZBoZ8bcshTHqOYJYlrqzRK5hagpagky5o4HfCzzd1TRkXPMFq6cKk9rGmA=="
    }
  }
}
"#;

const EMPTY_TRIVYIGNORE: &str = "vulnerabilities: []\nlicenses: []\n";

const CLEAN_PACKAGE_LOCK: &str = r#"{
  "name": "clean-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {}
}
"#;

const ACCEPTED_LODASH_TRIVYIGNORE: &str = r#"vulnerabilities:
  - id: CVE-2020-28500
    statement: "Test acceptance"
  - id: CVE-2021-23337
    statement: "Test acceptance"
  - id: CVE-2025-13465
    statement: "Test acceptance"
  - id: CVE-2026-4800
    statement: "Test acceptance"
licenses: []
"#;

const ACCEPTED_HIGH_ONLY_LODASH_TRIVYIGNORE: &str = r#"vulnerabilities:
  - id: CVE-2021-23337
    statement: "Ignore high severity to leave only medium CVEs unignored"
  - id: CVE-2025-13465
    statement: "Ignore other CVEs"
  - id: CVE-2026-4800
    statement: "Ignore other CVEs"
licenses: []
"#;

#[test]
fn scan_clean_repository_exits_zero() {
    let _guard = lock_trivy();
    let output = bin()
        .args(["scan", "."])
        .current_dir(repo_root())
        .output()
        .expect("failed to run scan");

    assert!(
        output.status.success(),
        "expected clean repo scan to exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("==>"),
        "expected tier headers on stderr, not stdout:\n{stdout}"
    );
    let repo_idx = stderr
        .find("==> Repository scan (trivy fs --scanners vuln,license):")
        .expect("expected repository tier header in stderr");
    let toolchain_idx = stderr
        .find("==> Toolchain sidecar scan (.config/mise/locks/, trivy fs --scanners vuln)")
        .expect("expected layered .config/mise/locks/ toolchain tier header in stderr");
    let summary_idx = stderr
        .find(
            "==> Scan summary: repo=clean, toolchain=clean, gobinary=skipped -> clean (exit code 0)",
        )
        .expect("expected scan summary line in stderr");
    assert!(repo_idx < toolchain_idx && toolchain_idx < summary_idx);
    assert!(
        !stderr.contains("warning: omitting unconfigured tool"),
        "expected no stale-tool warnings on repository itself:\n{stderr}"
    );
}

#[test]
fn scan_detects_vulnerable_repo_dependency_and_respects_trivyignore() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();

    let vuln_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan");
    assert_eq!(vuln_output.status.code(), Some(1));

    let custom_exit = bin()
        .arg("scan")
        .arg(tmp.path())
        .args(["--exit-code", "2"])
        .output()
        .expect("failed to run scan with custom exit code");
    assert_eq!(custom_exit.status.code(), Some(2));

    fs::write(
        tmp.path().join(".trivyignore.yaml"),
        ACCEPTED_LODASH_TRIVYIGNORE,
    )
    .unwrap();

    let ignored_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan with trivyignore");
    assert!(
        ignored_output.status.success(),
        "expected exit 0 when vulnerabilities are accepted in .trivyignore.yaml, got {:?}",
        ignored_output.status.code()
    );
}

#[test]
fn scan_detects_vulnerable_sidecar_lockfile_and_respects_trivyignore() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    let sidecar_dir = tmp.path().join(".mise/locks/npm-vuln/1.0.0");
    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();

    let vuln_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan");
    assert_eq!(vuln_output.status.code(), Some(1));

    fs::write(
        tmp.path().join(".trivyignore.yaml"),
        ACCEPTED_LODASH_TRIVYIGNORE,
    )
    .unwrap();

    let ignored_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan with trivyignore");
    assert!(
        ignored_output.status.success(),
        "expected exit 0 when sidecar vulnerabilities are accepted in .trivyignore.yaml, got {:?}",
        ignored_output.status.code()
    );
}

#[test]
fn scan_forwards_config_and_severity_flags_to_trivy() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    let sidecar_dir = tmp.path().join(".mise/locks/npm-vuln/1.0.0");
    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(
        tmp.path().join(".trivyignore.yaml"),
        ACCEPTED_HIGH_ONLY_LODASH_TRIVYIGNORE,
    )
    .unwrap();

    // With MEDIUM CVE-2020-28500 unignored, --severity MEDIUM,HIGH,CRITICAL fails.
    let medium_severity_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .args(["--severity", "MEDIUM,HIGH,CRITICAL"])
        .output()
        .expect("failed to run scan with --severity MEDIUM,HIGH,CRITICAL");
    assert_eq!(medium_severity_output.status.code(), Some(1));

    // Forwarding --severity HIGH,CRITICAL filters out MEDIUM CVE-2020-28500 across both repo and sidecar scans.
    let high_severity_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .args(["--severity", "HIGH,CRITICAL"])
        .output()
        .expect("failed to run scan with --severity HIGH,CRITICAL");
    assert!(
        high_severity_output.status.success(),
        "expected exit 0 when --severity HIGH,CRITICAL filters out medium CVE, got {:?}",
        high_severity_output.status.code()
    );

    // Forwarding --config with a custom trivy configuration setting severity to HIGH,CRITICAL also succeeds.
    let high_only_config = tmp.path().join("custom-trivy-high.yaml");
    fs::write(&high_only_config, "severity:\n  - HIGH\n  - CRITICAL\n").unwrap();

    let config_pass_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .arg("--config")
        .arg(&high_only_config)
        .output()
        .expect("failed to run scan with --config high-only");
    assert!(
        config_pass_output.status.success(),
        "expected exit 0 when --config restricts severity to HIGH,CRITICAL, got {:?}",
        config_pass_output.status.code()
    );

    let medium_config = tmp.path().join("custom-trivy-medium.yaml");
    fs::write(
        &medium_config,
        "severity:\n  - MEDIUM\n  - HIGH\n  - CRITICAL\n",
    )
    .unwrap();

    let config_fail_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .arg("--config")
        .arg(&medium_config)
        .output()
        .expect("failed to run scan with --config medium");
    assert_eq!(config_fail_output.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn scan_include_gobinary_scans_active_mise_install_paths() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = lock_trivy();

    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let fake_bin = tmp.path().join("bin");
    let fake_install = tmp.path().join("mise-installs/vuln-tool/1.0.0");

    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    let lodash_dir = fake_install.join("lib/node_modules/lodash");
    fs::create_dir_all(&lodash_dir).unwrap();

    fs::write(
        lodash_dir.join("package.json"),
        r#"{"name":"lodash","version":"4.17.20"}"#,
    )
    .unwrap();

    let mock_mise = fake_bin.join("mise");
    let script = format!(
        "#!/bin/sh\ncat <<'JSON'\n{{\"vuln-tool\": [{{\"install_path\": \"{}\", \"installed\": true, \"active\": true}}]}}\nJSON\n",
        fake_install.display()
    );
    fs::write(&mock_mise, script).unwrap();
    let mut perms = fs::metadata(&mock_mise).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_mise, perms).unwrap();

    let path_env = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let default_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("failed to run default scan");
    assert!(
        default_output.status.success(),
        "default scan should not scan mise install directories"
    );

    let gobinary_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .arg("--include-gobinary")
        .env("PATH", &path_env)
        .output()
        .expect("failed to run --include-gobinary scan");
    assert_eq!(
        gobinary_output.status.code(),
        Some(1),
        "expected --include-gobinary to scan active mise install directory and detect findings"
    );
    let gobinary_stderr = String::from_utf8_lossy(&gobinary_output.stderr);
    assert!(
        gobinary_stderr.contains(
            "==> Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln):"
        ),
        "expected binary toolchain header in stderr:\n{gobinary_stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&default_output.stderr).contains("==> Binary toolchain scan"),
        "default scan should not print binary toolchain header"
    );
}

#[cfg(unix)]
#[test]
fn scan_repo_only_skips_toolchain_and_gobinary() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = lock_trivy();

    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let sidecar_dir = project_dir.join(".mise/locks/npm-vuln/1.0.0");
    let fake_bin = tmp.path().join("bin");
    let fake_install = tmp.path().join("mise-installs/vuln-tool/1.0.0");
    let lodash_dir = fake_install.join("lib/node_modules/lodash");

    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::create_dir_all(&lodash_dir).unwrap();

    fs::write(project_dir.join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(project_dir.join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();
    fs::write(
        lodash_dir.join("package.json"),
        r#"{"name":"lodash","version":"4.17.20"}"#,
    )
    .unwrap();

    let mock_mise = fake_bin.join("mise");
    let script = format!(
        "#!/bin/sh\ncat <<'JSON'\n{{\"vuln-tool\": [{{\"install_path\": \"{}\", \"installed\": true, \"active\": true}}]}}\nJSON\n",
        fake_install.display()
    );
    fs::write(&mock_mise, script).unwrap();
    let mut perms = fs::metadata(&mock_mise).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_mise, perms).unwrap();

    let path_env = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // Default scan fails due to vulnerable sidecar lockfile.
    let default_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("failed to run default scan");
    assert_eq!(default_output.status.code(), Some(1));

    // --repo-only skips both .mise/locks/ and --include-gobinary.
    let repo_only_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .args(["--repo-only", "--include-gobinary"])
        .env("PATH", &path_env)
        .output()
        .expect("failed to run --repo-only scan");
    assert!(
        repo_only_output.status.success(),
        "expected --repo-only scan to exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        repo_only_output.status.code(),
        String::from_utf8_lossy(&repo_only_output.stdout),
        String::from_utf8_lossy(&repo_only_output.stderr)
    );

    let stderr = String::from_utf8_lossy(&repo_only_output.stderr);
    assert!(stderr.contains("==> Repository scan (trivy fs --scanners vuln,license):"));
    assert!(!stderr.contains("==> Toolchain sidecar scan"));
    assert!(!stderr.contains("==> Binary toolchain scan"));
    assert!(stderr.contains(
        "==> Scan summary: repo=clean, toolchain=skipped, gobinary=skipped -> clean (exit code 0)"
    ));

    // When the repository itself has findings, --repo-only still detects them.
    fs::write(project_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    let repo_vuln_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .arg("--repo-only")
        .output()
        .expect("failed to run --repo-only scan on vulnerable repo");
    assert_eq!(repo_vuln_output.status.code(), Some(1));
    let vuln_stderr = String::from_utf8_lossy(&repo_vuln_output.stderr);
    assert!(vuln_stderr.contains(
        "==> Scan summary: repo=findings, toolchain=skipped, gobinary=skipped -> findings detected (exit code 1)"
    ));
}

#[cfg(unix)]
#[test]
fn scan_toolchain_only_skips_repo_and_runs_sidecar_and_gobinary() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = lock_trivy();

    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let sidecar_dir = project_dir.join(".mise/locks/npm-clean/1.0.0");
    let fake_bin = tmp.path().join("bin");
    let fake_install = tmp.path().join("mise-installs/vuln-tool/1.0.0");
    let lodash_dir = fake_install.join("lib/node_modules/lodash");

    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::create_dir_all(&lodash_dir).unwrap();

    // Repo root is vulnerable, while sidecar lockfile is clean.
    fs::write(project_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(project_dir.join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();
    fs::write(
        lodash_dir.join("package.json"),
        r#"{"name":"lodash","version":"4.17.20"}"#,
    )
    .unwrap();

    let mock_mise = fake_bin.join("mise");
    let script = format!(
        "#!/bin/sh\ncat <<'JSON'\n{{\"vuln-tool\": [{{\"install_path\": \"{}\", \"installed\": true, \"active\": true}}]}}\nJSON\n",
        fake_install.display()
    );
    fs::write(&mock_mise, script).unwrap();
    let mut perms = fs::metadata(&mock_mise).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_mise, perms).unwrap();

    let path_env = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // --toolchain-only skips the vulnerable repo root and passes on the clean sidecar.
    let clean_toolchain_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .arg("--toolchain-only")
        .env("PATH", &path_env)
        .output()
        .expect("failed to run --toolchain-only scan");
    assert!(
        clean_toolchain_output.status.success(),
        "expected --toolchain-only to skip repo findings and exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        clean_toolchain_output.status.code(),
        String::from_utf8_lossy(&clean_toolchain_output.stdout),
        String::from_utf8_lossy(&clean_toolchain_output.stderr)
    );
    let clean_stderr = String::from_utf8_lossy(&clean_toolchain_output.stderr);
    assert!(!clean_stderr.contains("==> Repository scan"));
    assert!(
        clean_stderr
            .contains("==> Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")
    );
    assert!(!clean_stderr.contains("==> Binary toolchain scan"));
    assert!(clean_stderr.contains(
        "==> Scan summary: repo=skipped, toolchain=clean, gobinary=skipped -> clean (exit code 0)"
    ));

    // --toolchain-only --include-gobinary runs both sidecar and gobinary while still skipping repo.
    let gobinary_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .args(["--toolchain-only", "--include-gobinary"])
        .env("PATH", &path_env)
        .output()
        .expect("failed to run --toolchain-only --include-gobinary scan");
    assert_eq!(gobinary_output.status.code(), Some(1));
    let gobinary_stderr = String::from_utf8_lossy(&gobinary_output.stderr);
    assert!(!gobinary_stderr.contains("==> Repository scan"));
    assert!(
        gobinary_stderr
            .contains("==> Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")
    );
    assert!(
        gobinary_stderr.contains(
            "==> Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln):"
        )
    );
    assert!(gobinary_stderr.contains(
        "==> Scan summary: repo=skipped, toolchain=clean, gobinary=findings (1 path) -> findings detected (exit code 1)"
    ));

    // Vulnerable sidecar lockfile fails under --toolchain-only even when repo is clean.
    fs::write(project_dir.join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    let vuln_sidecar_output = bin()
        .arg("scan")
        .arg(&project_dir)
        .arg("--toolchain-only")
        .output()
        .expect("failed to run --toolchain-only on vulnerable sidecar");
    assert_eq!(vuln_sidecar_output.status.code(), Some(1));
    let vuln_stderr = String::from_utf8_lossy(&vuln_sidecar_output.stderr);
    assert!(!vuln_stderr.contains("==> Repository scan"));
    assert!(vuln_stderr.contains(
        "==> Scan summary: repo=skipped, toolchain=findings, gobinary=skipped -> findings detected (exit code 1)"
    ));
}

#[test]
fn scan_repo_only_and_toolchain_only_fail_with_conflict() {
    let output = bin()
        .args(["scan", "--repo-only", "--toolchain-only"])
        .output()
        .expect("failed to execute conflicting flags");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--repo-only") && stderr.contains("--toolchain-only"),
        "expected clap conflict message mentioning both flags, got:\n{stderr}"
    );
}

#[test]
fn scan_succeeds_on_v4_lockfile_with_provenance_and_sidecars() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    let sidecar_dir = tmp.path().join(".mise/locks/npm-clean/1.0.0");
    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(tmp.path().join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();

    let v4_lock = r#"lockfile_version = 4

[[tools."npm:@sigstore/bundle"]]
version = "3.1.0"
backend = "npm:@sigstore/bundle"
specifiers = ["3.1.0"]
provenance = { repository = "github.com/sigstore/sigstore-js" }

[[tools."npm:clean"]]
version = "1.0.0"
aube = { path = ".mise/locks/npm-clean/1.0.0" }
backend = "npm:clean"
specifiers = ["1.0.0"]
provenance = { repository = "github.com/example/clean" }
"#;
    fs::write(tmp.path().join("mise.lock"), v4_lock).unwrap();

    let clean_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan on v4 lockfile");
    assert!(
        clean_output.status.success(),
        "expected scan on clean v4 lockfile to exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        clean_output.status.code(),
        String::from_utf8_lossy(&clean_output.stdout),
        String::from_utf8_lossy(&clean_output.stderr)
    );
    let clean_stderr = String::from_utf8_lossy(&clean_output.stderr);
    assert!(clean_stderr.contains(
        "==> Scan summary: repo=clean, toolchain=clean, gobinary=skipped -> clean (exit code 0)"
    ));

    // Confirm sidecar vulnerabilities are still detected when mise.lock is v4.
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    let vuln_output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("failed to run scan on vulnerable v4 sidecar");
    assert_eq!(vuln_output.status.code(), Some(1));
    let vuln_stderr = String::from_utf8_lossy(&vuln_output.stderr);
    assert!(vuln_stderr.contains(
        "==> Scan summary: repo=clean, toolchain=findings, gobinary=skipped -> findings detected (exit code 1)"
    ));
}

#[test]
fn scan_skips_config_mise_locks_in_repo_tier_and_scans_them_in_toolchain_tier() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    let sidecar_dir = tmp.path().join(".config/mise/locks/npm-vuln/1.0.0");
    fs::create_dir_all(tmp.path().join(".config/mise/conf.d")).unwrap();
    fs::create_dir_all(&sidecar_dir).unwrap();

    fs::write(tmp.path().join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();
    fs::write(
        tmp.path().join(".config/mise/conf.d/tools.toml"),
        "[tools]\n\"npm:vuln\" = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join(".config/mise/mise.lock"),
        r#"lockfile_version = 3

[[tools."npm:vuln"]]
version = "1.0.0"
aube = { path = "locks/npm-vuln/1.0.0" }
backend = "npm:vuln"
"#,
    )
    .unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();

    // Tier 1 (--repo-only) must skip `.config/mise/locks/` so it is not scanned as repo deps.
    let repo_only = bin()
        .args(["scan", "--repo-only"])
        .arg(tmp.path())
        .output()
        .expect("run --repo-only scan");
    assert!(
        repo_only.status.success(),
        "expected --repo-only to skip .config/mise/locks/, stderr:\n{}",
        String::from_utf8_lossy(&repo_only.stderr)
    );
    assert!(String::from_utf8_lossy(&repo_only.stderr).contains(
        "==> Scan summary: repo=clean, toolchain=skipped, gobinary=skipped -> clean (exit code 0)"
    ));

    // Full scan scans `.config/mise/locks/` in Tier 2 (toolchain) and detects the finding.
    let full = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("run full scan");
    assert_eq!(full.status.code(), Some(1));
    let full_stderr = String::from_utf8_lossy(&full.stderr);
    assert!(
        full_stderr
            .contains("==> Toolchain sidecar scan (.config/mise/locks/, trivy fs --scanners vuln)")
    );
    assert!(full_stderr.contains(
        "==> Scan summary: repo=clean, toolchain=findings, gobinary=skipped -> findings detected (exit code 1)"
    ));
}

#[test]
fn scan_warns_and_omits_stale_lockfile_tool_and_its_vulnerable_sidecar() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    let stale_sidecar = tmp.path().join(".mise/locks/npm-stale/1.0.0");
    let active_sidecar = tmp.path().join(".mise/locks/npm-clean/1.0.0");
    fs::create_dir_all(&stale_sidecar).unwrap();
    fs::create_dir_all(&active_sidecar).unwrap();

    fs::write(tmp.path().join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();
    fs::write(stale_sidecar.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(active_sidecar.join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();

    fs::write(
        tmp.path().join("mise.toml"),
        "[tools]\n\"npm:clean\" = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("mise.lock"),
        r#"lockfile_version = 3

[[tools."npm:clean"]]
version = "1.0.0"
aube = { path = ".mise/locks/npm-clean/1.0.0" }
backend = "npm:clean"

[[tools."npm:stale"]]
version = "1.0.0"
aube = { path = ".mise/locks/npm-stale/1.0.0" }
backend = "npm:stale"
"#,
    )
    .unwrap();

    let output = bin()
        .arg("scan")
        .arg(tmp.path())
        .output()
        .expect("run scan with stale tool");
    assert!(
        output.status.success(),
        "expected stale tool's vulnerable sidecar to be omitted so scan exits 0, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: omitting unconfigured tool `npm:stale` from `mise.lock`"),
        "expected warning for omitted stale tool on stderr:\n{stderr}"
    );
    assert!(stderr.contains(
        "==> Scan summary: repo=clean, toolchain=clean, gobinary=skipped -> clean (exit code 0)"
    ));
}

#[test]
fn scan_include_local_and_custom_mise_flags() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().unwrap();
    let local_sidecar = tmp.path().join(".mise/locks/npm-localvuln/1.0.0");
    let custom_sidecar = tmp.path().join("custom/locks/npm-customvuln/1.0.0");
    fs::create_dir_all(&local_sidecar).unwrap();
    fs::create_dir_all(&custom_sidecar).unwrap();

    fs::write(tmp.path().join("package-lock.json"), CLEAN_PACKAGE_LOCK).unwrap();
    fs::write(tmp.path().join(".trivyignore.yaml"), EMPTY_TRIVYIGNORE).unwrap();
    fs::write(local_sidecar.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(custom_sidecar.join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();

    fs::write(
        tmp.path().join("mise.toml"),
        "[tools]\nrust = \"1.100.0\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("mise.lock"),
        "lockfile_version = 3\n\n[[tools.rust]]\nversion = \"1.100.0\"\nbackend = \"core:rust\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("mise.local.toml"),
        "[tools]\n\"npm:localvuln\" = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("mise.local.lock"),
        r#"lockfile_version = 3

[[tools."npm:localvuln"]]
version = "1.0.0"
aube = { path = ".mise/locks/npm-localvuln/1.0.0" }
backend = "npm:localvuln"
"#,
    )
    .unwrap();

    // Without --include-local, local tool & sidecar are excluded; scan exits 0.
    // Note: pass `-- --skip-dirs custom/locks` so the custom dir isn't scanned as repo deps in this check.
    let without_local = bin()
        .args(["scan", "--toolchain-only"])
        .arg(tmp.path())
        .output()
        .expect("run scan without --include-local");
    assert!(
        without_local.status.success(),
        "expected local sidecar to be excluded by default, stderr:\n{}",
        String::from_utf8_lossy(&without_local.stderr)
    );

    // With --include-local, mise.local.toml + mise.local.lock are included and vulnerable sidecar is caught.
    let with_local = bin()
        .args(["scan", "--toolchain-only", "--include-local"])
        .arg(tmp.path())
        .output()
        .expect("run scan with --include-local");
    assert_eq!(
        with_local.status.code(),
        Some(1),
        "expected --include-local to scan mise.local.lock sidecar and find CVE"
    );

    // Custom --mise-config and --mise-lock discover custom lockfile + its sidecar, and Tier 1 skips custom/locks.
    fs::write(
        tmp.path().join("custom/tools.toml"),
        "[tools]\n\"npm:customvuln\" = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("custom/custom.lock"),
        r#"lockfile_version = 3

[[tools."npm:customvuln"]]
version = "1.0.0"
aube = { path = "locks/npm-customvuln/1.0.0" }
backend = "npm:customvuln"
"#,
    )
    .unwrap();

    let custom_repo_only = bin()
        .args([
            "scan",
            "--repo-only",
            "--mise-config",
            "custom/tools.toml",
            "--mise-lock",
            "custom/custom.lock",
        ])
        .arg(tmp.path())
        .output()
        .expect("run repo-only scan with custom mise flags");
    assert!(
        custom_repo_only.status.success(),
        "expected Tier 1 repo scan to skip custom/locks, stderr:\n{}",
        String::from_utf8_lossy(&custom_repo_only.stderr)
    );

    let custom_full = bin()
        .args([
            "scan",
            "--mise-config",
            "custom/tools.toml",
            "--mise-lock",
            "custom/custom.lock",
        ])
        .arg(tmp.path())
        .output()
        .expect("run full scan with custom mise flags");
    assert_eq!(custom_full.status.code(), Some(1));
    let custom_stderr = String::from_utf8_lossy(&custom_full.stderr);
    assert!(custom_stderr.contains(
        "==> Scan summary: repo=clean, toolchain=findings, gobinary=skipped -> findings detected (exit code 1)"
    ));
}
