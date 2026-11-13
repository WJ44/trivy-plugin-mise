use std::process::Command;

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
fn sbom_unimplemented_exits_nonzero() {
    let output = bin().arg("sbom").output().expect("failed to run binary");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not yet implemented"));
}
