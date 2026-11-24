use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;
use tempfile::TempDir;

// Serialize Trivy-invoking tests so concurrent threads do not race to download
// or lock the shared Trivy cache on fresh CI runners.
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

fn copy_harness_mise_lock_as_root(dst_dir: &Path) {
    let raw = fs::read_to_string(repo_root().join(".config/mise/mise.lock")).unwrap();
    let adjusted = raw.replace("path = \"locks/", "path = \".mise/locks/");
    fs::write(dst_dir.join("mise.lock"), adjusted).unwrap();
    copy_dir_all(
        &repo_root().join(".config/mise/locks"),
        &dst_dir.join(".mise/locks"),
    );
}

fn collect_purls(components: &[Value]) -> HashSet<String> {
    components
        .iter()
        .filter_map(|c| c.get("purl").and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[test]
fn sbom_formulation_mode_on_repo_with_app_lockfile_and_mise_lock() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().expect("create temp dir");
    let project_dir = tmp.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();
    fs::copy(
        repo_root().join("Cargo.toml"),
        project_dir.join("Cargo.toml"),
    )
    .unwrap();
    fs::copy(
        repo_root().join("Cargo.lock"),
        project_dir.join("Cargo.lock"),
    )
    .unwrap();
    copy_dir_all(&repo_root().join("src"), &project_dir.join("src"));
    copy_harness_mise_lock_as_root(&project_dir);

    let output_path = tmp.path().join("sbom.cdx.json");

    let output = bin()
        .args([
            "sbom",
            "--platform",
            "linux-x64",
            "--output",
            output_path.to_str().unwrap(),
            project_dir.to_str().unwrap(),
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

    assert_eq!(
        bom["$schema"],
        "http://cyclonedx.org/schema/bom-1.7.schema.json"
    );
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

    // formulation[] contains mise.lock tools (with hashes & mise:* props), .mise/locks/ transitive deps,
    // and workflow metadata describing the mise toolchain.
    let formulation = bom["formulation"]
        .as_array()
        .expect("formulation[] should be populated");
    assert!(!formulation.is_empty());
    assert_eq!(formulation[0]["bom-ref"], "mise-toolchain");

    let workflows = formulation[0]["workflows"]
        .as_array()
        .expect("formulation[0].workflows should be an array");
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0]["bom-ref"], "workflow:mise-toolchain");
    assert_eq!(workflows[0]["uid"], "mise-toolchain");
    assert_eq!(workflows[0]["name"], "mise development and build toolchain");
    assert!(
        workflows[0]["description"]
            .as_str()
            .is_some_and(|d| !d.is_empty()),
        "expected non-empty workflow description"
    );
    assert_eq!(
        workflows[0]["taskTypes"],
        serde_json::json!(["build", "lint", "test", "scan"])
    );

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
        formula_purls.contains("pkg:npm/prettier@3.10.1"),
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

    // Top-level dependencies[] includes both shipped Cargo graph and mise-toolchain graph without polluting root.
    let dependencies = bom["dependencies"]
        .as_array()
        .expect("top-level dependencies[] should be an array");
    let dep_refs: Vec<&str> = dependencies
        .iter()
        .filter_map(|d| d.get("ref").and_then(Value::as_str))
        .collect();
    let unique_dep_refs: HashSet<&str> = dep_refs.iter().copied().collect();
    assert_eq!(
        dep_refs.len(),
        unique_dep_refs.len(),
        "dependencies[].ref entries must be unique"
    );

    let repo_root_ref = bom["metadata"]["component"]["bom-ref"]
        .as_str()
        .expect("metadata.component.bom-ref should be present");
    let repo_root_dep = dependencies
        .iter()
        .find(|d| d.get("ref").and_then(Value::as_str) == Some(repo_root_ref))
        .expect("repository root should have a dependencies[] entry");
    let repo_root_depends_on: Vec<&str> = repo_root_dep["dependsOn"]
        .as_array()
        .expect("repository root dependsOn should be an array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !repo_root_depends_on.contains(&"mise-toolchain"),
        "repository root dependsOn must not include mise-toolchain"
    );
    assert!(
        !repo_root_depends_on
            .iter()
            .any(|r| r.starts_with("pkg:npm/prettier@")
                || r.starts_with("pkg:github/evilmartians/lefthook@")),
        "repository root dependsOn must not include mise toolchain nodes"
    );

    let toolchain_dep = dependencies
        .iter()
        .find(|d| d.get("ref").and_then(Value::as_str) == Some("mise-toolchain"))
        .expect("dependencies[] should include mise-toolchain root");
    let toolchain_depends_on: Vec<&str> = toolchain_dep["dependsOn"]
        .as_array()
        .expect("mise-toolchain dependsOn should be an array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        toolchain_depends_on.contains(&"pkg:npm/prettier@3.10.1"),
        "mise-toolchain dependsOn should include prettier"
    );
    assert!(
        toolchain_depends_on.contains(&"pkg:npm/%40commitlint/cli@21.2.4"),
        "mise-toolchain dependsOn should include @commitlint/cli"
    );
    assert!(
        toolchain_depends_on
            .iter()
            .any(|r| r.starts_with("pkg:github/evilmartians/lefthook@2.2.2")),
        "mise-toolchain dependsOn should include lefthook"
    );

    let commitlint_dep = dependencies
        .iter()
        .find(|d| d.get("ref").and_then(Value::as_str) == Some("pkg:npm/%40commitlint/cli@21.2.4"))
        .expect("expected transitive dependency entry for @commitlint/cli");
    assert!(
        !commitlint_dep["dependsOn"].as_array().unwrap().is_empty(),
        "expected @commitlint/cli to retain transitive dependsOn edges"
    );

    // Formulation output must be accepted by `trivy sbom` without regressions.
    let trivy_out = Command::new("trivy")
        .args(["sbom", "--quiet", output_path.to_str().unwrap()])
        .output()
        .expect("run trivy sbom on formulation output");
    assert!(
        trivy_out.status.success(),
        "trivy sbom failed on formulation output with status {}: {}",
        trivy_out.status,
        String::from_utf8_lossy(&trivy_out.stderr)
    );
}

#[test]
fn sbom_formulation_mode_on_mise_only_repo_has_empty_components_and_populated_formulation() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().expect("create temp mise-only repo");
    copy_harness_mise_lock_as_root(tmp.path());

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

    assert_eq!(bom["formulation"][0]["bom-ref"], "mise-toolchain");
    let workflows = bom["formulation"][0]["workflows"]
        .as_array()
        .expect("formulation[0].workflows should be populated");
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0]["bom-ref"], "workflow:mise-toolchain");
    assert_eq!(workflows[0]["uid"], "mise-toolchain");
    assert_eq!(
        workflows[0]["taskTypes"],
        serde_json::json!(["build", "lint", "test", "scan"])
    );

    let formula_components = bom["formulation"][0]["components"]
        .as_array()
        .expect("formulation[0].components should be populated");
    assert!(
        formula_components.len() > 15,
        "expected mise.lock + .mise/locks/ components in formulation[]"
    );

    let dependencies = bom["dependencies"]
        .as_array()
        .expect("dependencies[] should be populated on mise-only repo");
    assert!(
        dependencies.len() > 2,
        "expected root, mise-toolchain, and transitive entries in dependencies[], got {}",
        dependencies.len()
    );

    let repo_root_ref = bom["metadata"]["component"]["bom-ref"]
        .as_str()
        .expect("metadata.component.bom-ref should be present");
    let repo_root_dep = dependencies
        .iter()
        .find(|d| d.get("ref").and_then(Value::as_str) == Some(repo_root_ref))
        .expect("empty repo root should remain in dependencies[]");
    assert!(
        repo_root_dep["dependsOn"].as_array().unwrap().is_empty(),
        "empty repo root dependsOn must remain empty"
    );

    let toolchain_dep = dependencies
        .iter()
        .find(|d| d.get("ref").and_then(Value::as_str) == Some("mise-toolchain"))
        .expect("mise-toolchain root should be in dependencies[]");
    let toolchain_depends_on: Vec<&str> = toolchain_dep["dependsOn"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(toolchain_depends_on.contains(&"pkg:npm/%40commitlint/cli@21.2.4"));

    let trivy_out = Command::new("trivy")
        .args(["sbom", "--quiet", output_path.to_str().unwrap()])
        .output()
        .expect("run trivy sbom on mise-only formulation output");
    assert!(
        trivy_out.status.success(),
        "trivy sbom failed on mise-only formulation output with status {}: {}",
        trivy_out.status,
        String::from_utf8_lossy(&trivy_out.stderr)
    );
}

#[test]
fn sbom_components_mode_emits_valid_cyclonedx_accepted_by_trivy_sbom() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().expect("create temp dir");
    let project_dir = tmp.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();
    fs::copy(
        repo_root().join("Cargo.toml"),
        project_dir.join("Cargo.toml"),
    )
    .unwrap();
    fs::copy(
        repo_root().join("Cargo.lock"),
        project_dir.join("Cargo.lock"),
    )
    .unwrap();
    copy_harness_mise_lock_as_root(&project_dir);

    let output_path = tmp.path().join("sbom-tools.cdx.json");

    let output = bin()
        .args([
            "sbom",
            "--mode",
            "components",
            "--output",
            output_path.to_str().unwrap(),
            project_dir.to_str().unwrap(),
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
    assert!(purls.contains("pkg:npm/prettier@3.10.1"));
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

#[test]
fn sbom_succeeds_on_v4_lockfile_with_provenance_and_sidecars() {
    let _guard = lock_trivy();
    let tmp = TempDir::new().expect("create temp dir");
    let project_dir = tmp.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();

    // Start from the repo's harness lockfile & sidecars, upgrade header to lockfile_version = 4,
    // and add a v4 npm tool entry with `provenance = { repository = "..." }`.
    copy_harness_mise_lock_as_root(&project_dir);
    let raw = fs::read_to_string(project_dir.join("mise.lock")).unwrap();
    let v4_lock = format!(
        "{}\n[[tools.\"npm:@sigstore/bundle\"]]\nversion = \"3.1.0\"\nbackend = \"npm:@sigstore/bundle\"\nspecifiers = [\"3.1.0\"]\nprovenance = {{ repository = \"github.com/sigstore/sigstore-js\" }}\n",
        raw.replacen("lockfile_version = 3", "lockfile_version = 4", 1)
    );
    fs::write(project_dir.join("mise.lock"), v4_lock).unwrap();

    // Formulation mode
    let formulation_out = tmp.path().join("sbom-v4-formulation.cdx.json");
    let output = bin()
        .args([
            "sbom",
            "--platform",
            "linux-x64",
            "--output",
            formulation_out.to_str().unwrap(),
            project_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run sbom on v4 lockfile");
    assert!(
        output.status.success(),
        "sbom formulation mode failed on v4 lockfile: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bom: Value = serde_json::from_str(&fs::read_to_string(&formulation_out).unwrap()).unwrap();
    let formula_components = bom["formulation"][0]["components"]
        .as_array()
        .expect("formulation[0].components should be populated");
    let bundle = formula_components
        .iter()
        .find(|c| c.get("purl").and_then(Value::as_str) == Some("pkg:npm/%40sigstore/bundle@3.1.0"))
        .expect("v4 npm:@sigstore/bundle component should be present");
    assert!(
        bundle["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "mise:provenance:repository"
                && p["value"] == "github.com/sigstore/sigstore-js"),
        "expected mise:provenance:repository property on @sigstore/bundle"
    );

    let trivy_formulation = Command::new("trivy")
        .args(["sbom", "--quiet", formulation_out.to_str().unwrap()])
        .output()
        .expect("run trivy sbom on v4 formulation output");
    assert!(
        trivy_formulation.status.success(),
        "trivy sbom failed on v4 formulation output: {}",
        String::from_utf8_lossy(&trivy_formulation.stderr)
    );

    // Components mode
    let components_out = tmp.path().join("sbom-v4-components.cdx.json");
    let comp_output = bin()
        .args([
            "sbom",
            "--mode",
            "components",
            "--platform",
            "linux-x64",
            "--output",
            components_out.to_str().unwrap(),
            project_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run sbom --mode components on v4 lockfile");
    assert!(
        comp_output.status.success(),
        "sbom components mode failed on v4 lockfile: {}",
        String::from_utf8_lossy(&comp_output.stderr)
    );

    let comp_bom: Value =
        serde_json::from_str(&fs::read_to_string(&components_out).unwrap()).unwrap();
    let comp_purls = collect_purls(comp_bom["components"].as_array().unwrap());
    assert!(comp_purls.contains("pkg:npm/%40sigstore/bundle@3.1.0"));
    assert!(comp_purls.contains("pkg:npm/prettier@3.10.1"));

    let trivy_components = Command::new("trivy")
        .args(["sbom", "--quiet", components_out.to_str().unwrap()])
        .output()
        .expect("run trivy sbom on v4 components output");
    assert!(
        trivy_components.status.success(),
        "trivy sbom failed on v4 components output: {}",
        String::from_utf8_lossy(&trivy_components.stderr)
    );
}
