use std::fs;
use std::path::Path;
use std::process::Command;

const PLUGIN_YAML: &str = include_str!("../plugin.yaml");

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trivy-plugin-mise"))
}

#[test]
fn help_exits_zero() {
    let output = bin().arg("--help").output().expect("failed to run binary");
    assert!(output.status.success(), "expected exit 0 for --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sbom"));
    assert!(stdout.contains("scan"));
    assert!(stdout.contains("version"));
}

#[test]
fn version_exits_zero() {
    let output = bin()
        .arg("--version")
        .output()
        .expect("failed to run binary");
    assert!(output.status.success(), "expected exit 0 for --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn version_subcommand_exits_zero() {
    let output = bin().arg("version").output().expect("failed to run binary");
    assert!(output.status.success(), "expected exit 0 for version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn sbom_help_exits_zero() {
    let output = bin()
        .args(["sbom", "--help"])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success(), "expected exit 0 for sbom --help");
}

#[test]
fn scan_help_exits_zero() {
    let output = bin()
        .args(["scan", "--help"])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success(), "expected exit 0 for scan --help");
}

#[test]
fn plugin_yaml_follows_developer_guide_schema() {
    assert!(
        !PLUGIN_YAML.lines().any(|line| line.starts_with("usage:")),
        "plugin.yaml must not contain the deprecated top-level `usage:` field"
    );
    assert!(
        PLUGIN_YAML
            .lines()
            .any(|line| line.trim() == "output: false"),
        "plugin.yaml must keep `output: false`"
    );
    assert!(
        PLUGIN_YAML.contains("Usage:")
            && PLUGIN_YAML.contains("trivy mise [scan] [OPTIONS] [TARGET]")
            && PLUGIN_YAML.contains("trivy mise sbom [OPTIONS] [PATH]"),
        "plugin.yaml description must include Usage examples"
    );
}

#[test]
fn trivy_plugin_install_and_invoke_smoke() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let pkg_dir = temp.path().join("pkg");
    let xdg_dir = temp.path().join("xdg");
    fs::create_dir_all(&pkg_dir).expect("failed to create pkg dir");
    fs::create_dir_all(&xdg_dir).expect("failed to create xdg dir");

    let compiled_bin = Path::new(env!("CARGO_BIN_EXE_trivy-plugin-mise"));
    let bin_name = if cfg!(windows) {
        "trivy-plugin-mise.exe"
    } else {
        "trivy-plugin-mise"
    };
    let staged_bin = pkg_dir.join(bin_name);
    fs::copy(compiled_bin, &staged_bin).expect("failed to copy compiled plugin binary");

    let (manifest_header, _) = PLUGIN_YAML
        .split_once("\nplatforms:\n")
        .expect("plugin.yaml should contain a platforms section");
    let local_manifest =
        format!("{manifest_header}\nplatforms:\n  - uri: ./{bin_name}\n    bin: ./{bin_name}\n");
    fs::write(pkg_dir.join("plugin.yaml"), local_manifest)
        .expect("failed to write local plugin.yaml");

    let archive_path = temp.path().join("trivy-plugin-mise.tar.gz");
    let tar_status = Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&pkg_dir)
        .arg("plugin.yaml")
        .arg(bin_name)
        .status()
        .expect("failed to run tar to package plugin archive");
    assert!(tar_status.success(), "tar archive creation failed");

    let install_output = Command::new("trivy")
        .env("XDG_DATA_HOME", &xdg_dir)
        .args(["plugin", "install"])
        .arg(&archive_path)
        .output()
        .expect("failed to execute `trivy plugin install`");
    assert!(
        install_output.status.success(),
        "trivy plugin install failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&install_output.stdout),
        String::from_utf8_lossy(&install_output.stderr)
    );

    let list_output = Command::new("trivy")
        .env("XDG_DATA_HOME", &xdg_dir)
        .args(["plugin", "list"])
        .output()
        .expect("failed to execute `trivy plugin list`");
    assert!(list_output.status.success(), "trivy plugin list failed");
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        list_stdout.contains("mise") && list_stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected `trivy plugin list` to report installed `mise` plugin, got:\n{list_stdout}"
    );

    let info_output = Command::new("trivy")
        .env("XDG_DATA_HOME", &xdg_dir)
        .args(["plugin", "info", "mise"])
        .output()
        .expect("failed to execute `trivy plugin info mise`");
    assert!(
        info_output.status.success(),
        "trivy plugin info mise failed"
    );
    let info_stdout = String::from_utf8_lossy(&info_output.stdout);
    assert!(
        info_stdout.contains("Usage:") && info_stdout.contains("trivy mise sbom"),
        "expected `trivy plugin info mise` to display Usage examples, got:\n{info_stdout}"
    );

    let version_output = Command::new("trivy")
        .env("XDG_DATA_HOME", &xdg_dir)
        .args(["mise", "--version"])
        .output()
        .expect("failed to execute `trivy mise --version`");
    assert!(
        version_output.status.success(),
        "trivy mise --version failed:\nstderr: {}",
        String::from_utf8_lossy(&version_output.stderr)
    );
    let version_stdout = String::from_utf8_lossy(&version_output.stdout);
    assert!(
        version_stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected version output to contain {}, got:\n{version_stdout}",
        env!("CARGO_PKG_VERSION")
    );

    let help_output = Command::new("trivy")
        .env("XDG_DATA_HOME", &xdg_dir)
        .args(["mise", "--help"])
        .output()
        .expect("failed to execute `trivy mise --help`");
    assert!(
        help_output.status.success(),
        "trivy mise --help failed:\nstderr: {}",
        String::from_utf8_lossy(&help_output.stderr)
    );
    let help_stdout = String::from_utf8_lossy(&help_output.stdout);
    assert!(help_stdout.contains("sbom"));
    assert!(help_stdout.contains("scan"));
}
