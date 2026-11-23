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
    let repo_idx = stdout
        .find("==> Repository scan (trivy fs --scanners vuln,license):")
        .expect("expected repository tier header in stdout");
    let toolchain_idx = stdout
        .find("==> Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")
        .expect("expected toolchain sidecar tier header in stdout");
    let summary_idx = stdout
        .find("==> Scan summary: repo=clean, toolchain=clean, gobinary=skipped -> clean (exit code 0)")
        .expect("expected scan summary line in stdout");
    assert!(repo_idx < toolchain_idx && toolchain_idx < summary_idx);
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
    let gobinary_stdout = String::from_utf8_lossy(&gobinary_output.stdout);
    assert!(
        gobinary_stdout.contains(
            "==> Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln):"
        ),
        "expected binary toolchain header in stdout:\n{gobinary_stdout}"
    );
    assert!(
        !String::from_utf8_lossy(&default_output.stdout).contains("==> Binary toolchain scan"),
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

    let stdout = String::from_utf8_lossy(&repo_only_output.stdout);
    assert!(stdout.contains("==> Repository scan (trivy fs --scanners vuln,license):"));
    assert!(!stdout.contains("==> Toolchain sidecar scan"));
    assert!(!stdout.contains("==> Binary toolchain scan"));
    assert!(stdout.contains(
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
    let vuln_stdout = String::from_utf8_lossy(&repo_vuln_output.stdout);
    assert!(vuln_stdout.contains(
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
    let clean_stdout = String::from_utf8_lossy(&clean_toolchain_output.stdout);
    assert!(!clean_stdout.contains("==> Repository scan"));
    assert!(
        clean_stdout
            .contains("==> Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")
    );
    assert!(!clean_stdout.contains("==> Binary toolchain scan"));
    assert!(clean_stdout.contains(
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
    let gobinary_stdout = String::from_utf8_lossy(&gobinary_output.stdout);
    assert!(!gobinary_stdout.contains("==> Repository scan"));
    assert!(
        gobinary_stdout
            .contains("==> Toolchain sidecar scan (.mise/locks/, trivy fs --scanners vuln)")
    );
    assert!(
        gobinary_stdout.contains(
            "==> Binary toolchain scan (--include-gobinary, trivy rootfs --scanners vuln):"
        )
    );
    assert!(gobinary_stdout.contains(
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
    let vuln_stdout = String::from_utf8_lossy(&vuln_sidecar_output.stdout);
    assert!(!vuln_stdout.contains("==> Repository scan"));
    assert!(vuln_stdout.contains(
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
