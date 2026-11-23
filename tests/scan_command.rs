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
}
