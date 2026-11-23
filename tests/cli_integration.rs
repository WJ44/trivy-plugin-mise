use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trivy-plugin-mise"))
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

const PLAIN_TRIVYIGNORE_ALL_LODASH: &str = r#"# Plain .trivyignore fallback format
CVE-2020-28500
CVE-2021-23337
CVE-2025-13465
CVE-2026-4800
"#;

const YAML_TRIVYIGNORE_EMPTY: &str = "vulnerabilities: []\nlicenses: []\n";

#[cfg(unix)]
fn install_mock_trivy(dir: &Path, script_body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(dir).unwrap();
    let mock_trivy = dir.join("trivy");
    fs::write(&mock_trivy, script_body).unwrap();
    let mut perms = fs::metadata(&mock_trivy).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_trivy, perms).unwrap();

    format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[cfg(unix)]
#[test]
fn default_command_runs_scan_and_forwards_short_flags_and_trivy_args() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let fake_bin = tmp.path().join("bin");
    let log_file = tmp.path().join("trivy-args.log");

    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("trivy.yaml"), "severity:\n  - CRITICAL\n").unwrap();
    fs::write(project_dir.join(".trivyignore"), "CVE-2020-28500\n").unwrap();

    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\nprintf 'TRIVY_STDOUT_PAYLOAD\\n'\n",
        log_file.display()
    );
    let path_env = install_mock_trivy(&fake_bin, &script);

    // Invoke without `scan` subcommand, passing target, short flags, new flags, and trailing `-- <TRIVY_ARGS>...`.
    let output = bin()
        .arg(&project_dir)
        .args([
            "-c",
            "custom-config.yaml",
            "-s",
            "HIGH,CRITICAL",
            "--ignore-unfixed",
            "--scanners",
            "vuln",
            "--",
            "--skip-db-update",
            "--offline-scan",
        ])
        .env("PATH", &path_env)
        .output()
        .expect("failed to run default scan invocation");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Tier progress headers and summary must be written to stderr, leaving stdout clean for Trivy output.
    assert!(
        !stdout.contains("==>"),
        "stdout must not contain tier progress headers:\n{stdout}"
    );
    assert!(
        stdout.contains("TRIVY_STDOUT_PAYLOAD"),
        "expected trivy stdout payload on stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("==> Repository scan (trivy fs --scanners vuln):"),
        "expected custom scanners in repository scan header on stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "==> Scan summary: repo=clean, toolchain=none, gobinary=skipped -> clean (exit code 0)"
        ),
        "expected scan summary on stderr:\n{stderr}"
    );

    let recorded = fs::read_to_string(&log_file).unwrap();
    assert!(
        recorded.contains("--scanners vuln"),
        "expected --scanners vuln forwarded to trivy:\n{recorded}"
    );
    assert!(
        recorded.contains("--config custom-config.yaml"),
        "expected -c/--config forwarded to trivy:\n{recorded}"
    );
    assert!(
        recorded.contains(&format!(
            "--ignorefile {}",
            project_dir.join(".trivyignore").display()
        )),
        "expected fallback .trivyignore forwarded to trivy:\n{recorded}"
    );
    assert!(
        recorded.contains("--severity HIGH,CRITICAL"),
        "expected -s/--severity forwarded to trivy:\n{recorded}"
    );
    assert!(
        recorded.contains("--ignore-unfixed"),
        "expected --ignore-unfixed forwarded to trivy:\n{recorded}"
    );
    assert!(
        recorded.contains("--skip-db-update --offline-scan"),
        "expected trailing -- <TRIVY_ARGS> forwarded to trivy:\n{recorded}"
    );
}

#[cfg(unix)]
#[test]
fn quiet_flag_suppresses_stderr_tier_headers_and_forwards_quiet_to_trivy() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let sidecar_dir = project_dir.join(".mise/locks/npm-tool/1.0.0");
    let fake_bin = tmp.path().join("bin");
    let log_file = tmp.path().join("trivy-quiet.log");

    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), "{}\n").unwrap();

    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\n",
        log_file.display()
    );
    let path_env = install_mock_trivy(&fake_bin, &script);

    let output = bin()
        .arg(&project_dir)
        .arg("-q")
        .env("PATH", &path_env)
        .output()
        .expect("failed to run quiet scan");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.is_empty(),
        "expected empty stdout under -q:\n{stdout}"
    );
    assert!(
        stderr.is_empty(),
        "expected -q/--quiet to suppress all ==> tier and summary headers on stderr:\n{stderr}"
    );

    let recorded = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(lines.len(), 2, "expected repo and sidecar trivy calls");
    for line in lines {
        assert!(
            line.contains("--quiet"),
            "expected --quiet forwarded to each trivy invocation:\n{line}"
        );
    }
}

#[cfg(unix)]
#[test]
fn sbom_forwards_trailing_trivy_args_to_trivy_fs() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("project");
    let sidecar_dir = project_dir.join(".mise/locks/npm-tool/1.0.0");
    let fake_bin = tmp.path().join("bin");
    let log_file = tmp.path().join("trivy-sbom.log");
    let output_path = tmp.path().join("sbom.cdx.json");

    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(sidecar_dir.join("package-lock.json"), "{}\n").unwrap();

    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\ncat <<'JSON'\n{{\"bomFormat\":\"CycloneDX\",\"specVersion\":\"1.6\",\"metadata\":{{\"component\":{{\"bom-ref\":\"root\"}}}},\"components\":[],\"dependencies\":[]}}\nJSON\n",
        log_file.display()
    );
    let path_env = install_mock_trivy(&fake_bin, &script);

    let output = bin()
        .args([
            "sbom",
            "--output",
            output_path.to_str().unwrap(),
            project_dir.to_str().unwrap(),
            "--",
            "--skip-db-update",
            "--offline-scan",
        ])
        .env("PATH", &path_env)
        .output()
        .expect("failed to run sbom with trailing trivy args");

    assert!(
        output.status.success(),
        "expected sbom to exit 0, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bom: Value = serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
    assert_eq!(bom["bomFormat"], "CycloneDX");
    assert_eq!(bom["specVersion"], "1.7");

    let recorded = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "expected sidecar and base repo trivy fs --format cyclonedx calls"
    );
    for line in lines {
        assert!(
            line.contains("--skip-db-update --offline-scan"),
            "expected trailing trivy args forwarded to trivy fs:\n{line}"
        );
    }
}

#[test]
fn scan_falls_back_to_plain_trivyignore_and_prefers_trivyignore_yaml() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("package-lock.json"), VULN_PACKAGE_LOCK).unwrap();
    fs::write(
        tmp.path().join(".trivyignore"),
        PLAIN_TRIVYIGNORE_ALL_LODASH,
    )
    .unwrap();

    // With only `.trivyignore` present, default scan discovers it and exits 0.
    let fallback_output = bin()
        .arg(tmp.path())
        .output()
        .expect("failed to run default scan with .trivyignore fallback");
    assert!(
        fallback_output.status.success(),
        "expected exit 0 when vulnerabilities are ignored via .trivyignore fallback, got {:?}\nstderr:\n{}",
        fallback_output.status.code(),
        String::from_utf8_lossy(&fallback_output.stderr)
    );

    // When `.trivyignore.yaml` is also present, it takes precedence over `.trivyignore`.
    fs::write(tmp.path().join(".trivyignore.yaml"), YAML_TRIVYIGNORE_EMPTY).unwrap();
    let yaml_precedence_output = bin()
        .arg(tmp.path())
        .output()
        .expect("failed to run scan with both .trivyignore.yaml and .trivyignore");
    assert_eq!(
        yaml_precedence_output.status.code(),
        Some(1),
        "expected .trivyignore.yaml to take precedence over .trivyignore"
    );
}
