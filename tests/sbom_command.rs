use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trivy-plugin-mise"))
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn collect_purls(components: &[Value]) -> HashSet<String> {
    components
        .iter()
        .filter_map(|c| c.get("purl").and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[test]
fn sbom_formulation_mode_on_repo_with_app_lockfile_and_mise_lock() {
    let tmp = TempDir::new().expect("create temp dir");
    let output_path = tmp.path().join("sbom.cdx.json");

    let output = bin()
        .args([
            "sbom",
            "--platform",
            "linux-x64",
            "--output",
            output_path.to_str().unwrap(),
            repo_root().to_str().unwrap(),
        ])
        .output()
        .expect("run sbom command");

    assert!(
        output.status.success(),
        "sbom failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file(), "output file should be created");

    let raw = fs::read_to_string(&output_path).unwrap();
    let bom: Value = serde_json::from_str(&raw).unwrap();

    assert_eq!(bom["bomFormat"], "CycloneDX");
    assert_eq!(bom["specVersion"], "1.7");

    // Top-level components[] contains shipped Cargo dependencies, NOT mise tools.
    let top_components = bom["components"]
        .as_array()
        .expect("top-level components[] should be an array");
    let top_purls = collect_purls(top_components);
    assert!(
        top_purls.iter().any(|p| p.starts_with("pkg:cargo/clap@")),
        "expected shipped Cargo dependency in top-level components[]"
    );
    assert!(
        !top_purls.iter().any(|p| p.starts_with("pkg:npm/prettier@")),
        "mise tool prettier must not leak into top-level components[]"
    );

    // formulation[] contains mise.lock tools (with hashes & mise:* props) and .mise/locks/ transitive deps.
    let formulation = bom["formulation"]
        .as_array()
        .expect("formulation[] should be populated");
    assert!(!formulation.is_empty());

    let formula_components = formulation[0]["components"]
        .as_array()
        .expect("formulation[0].components should be an array");
    let formula_purls = collect_purls(formula_components);

    assert!(
        formula_purls
            .iter()
            .any(|p| p.starts_with("pkg:github/evilmartians/lefthook@2.2.2")),
        "expected lefthook from mise.lock in formulation[]"
    );
    assert!(
        formula_purls.contains("pkg:npm/prettier@3.9.10"),
        "expected prettier in formulation[]"
    );
    assert!(
        formula_purls.contains("pkg:npm/%40commitlint/cli@21.2.4"),
        "expected @commitlint/cli in formulation[]"
    );
    assert!(
        formula_components.len() > 15,
        "expected transitive .mise/locks/ dependencies in formulation[], got {}",
        formula_components.len()
    );

    // Verify lefthook has platform hash and mise:* properties.
    let lefthook = formula_components
        .iter()
        .find(|c| {
            c.get("purl")
                .and_then(Value::as_str)
                .is_some_and(|p| p.starts_with("pkg:github/evilmartians/lefthook@2.2.2"))
        })
        .expect("lefthook component present");
    assert!(
        !lefthook["hashes"].as_array().unwrap().is_empty(),
        "expected platform hashes on lefthook when --platform linux-x64 is set"
    );
    assert!(
        lefthook["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "mise:tool" && p["value"] == "lefthook")
    );

    // Ensure unique bom-refs across formulation components.
    let bom_refs: Vec<&str> = formula_components
        .iter()
        .filter_map(|c| c.get("bom-ref").and_then(Value::as_str))
        .collect();
    let unique_refs: HashSet<&str> = bom_refs.iter().copied().collect();
    assert_eq!(bom_refs.len(), unique_refs.len(), "bom-refs must be unique");
}

#[test]
fn sbom_formulation_mode_on_mise_only_repo_has_empty_components_and_populated_formulation() {
    let tmp = TempDir::new().expect("create temp mise-only repo");
    fs::copy(repo_root().join("mise.lock"), tmp.path().join("mise.lock")).unwrap();
    copy_dir_all(
        &repo_root().join(".mise/locks"),
        &tmp.path().join(".mise/locks"),
    );

    let output_path = tmp.path().join("sbom.cdx.json");
    let output = bin()
        .args([
            "sbom",
            "--output",
            output_path.to_str().unwrap(),
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run sbom on mise-only repo");

    assert!(
        output.status.success(),
        "sbom failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bom: Value = serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
    assert_eq!(bom["bomFormat"], "CycloneDX");
    assert_eq!(bom["specVersion"], "1.7");

    let top_components = bom["components"].as_array().unwrap();
    assert!(
        top_components.is_empty(),
        "mise-only repo should have empty top-level components[], got {}",
        top_components.len()
    );

    let formula_components = bom["formulation"][0]["components"]
        .as_array()
        .expect("formulation[0].components should be populated");
    assert!(
        formula_components.len() > 15,
        "expected mise.lock + .mise/locks/ components in formulation[]"
    );
}

#[test]
fn sbom_components_mode_emits_valid_cyclonedx_accepted_by_trivy_sbom() {
    let tmp = TempDir::new().expect("create temp dir");
    let output_path = tmp.path().join("sbom-tools.cdx.json");

    let output = bin()
        .args([
            "sbom",
            "--mode",
            "components",
            "--output",
            output_path.to_str().unwrap(),
            repo_root().to_str().unwrap(),
        ])
        .output()
        .expect("run sbom --mode components");

    assert!(
        output.status.success(),
        "sbom --mode components failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bom: Value = serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
    assert_eq!(bom["bomFormat"], "CycloneDX");
    assert_eq!(bom["specVersion"], "1.7");
    assert!(bom.get("formulation").is_none());

    let components = bom["components"].as_array().unwrap();
    let purls = collect_purls(components);
    assert!(purls.contains("pkg:npm/prettier@3.9.10"));
    assert!(purls.contains("pkg:npm/%40commitlint/cli@21.2.4"));
    assert!(
        !purls.iter().any(|p| p.starts_with("pkg:cargo/clap@")),
        "standalone toolchain SBOM should not include shipped Cargo dependencies"
    );

    let dependencies = bom["dependencies"].as_array().unwrap();
    assert!(
        dependencies.len() > 1,
        "expected dependency graph entries in --mode components"
    );

    // Acceptance criterion: passing --mode components output to `trivy sbom` succeeds with exit code 0.
    let trivy_out = Command::new("trivy")
        .args(["sbom", "--quiet", output_path.to_str().unwrap()])
        .output()
        .expect("run trivy sbom");

    assert!(
        trivy_out.status.success(),
        "trivy sbom failed with status {}: {}",
        trivy_out.status,
        String::from_utf8_lossy(&trivy_out.stderr)
    );
}
