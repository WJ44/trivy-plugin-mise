use std::fs;
use std::path::Path;

use tempfile::TempDir;
use trivy_plugin_mise::mise_lock::MiseLock;
use trivy_plugin_mise::sidecar::{
    SidecarError, SidecarOptions, extract_sidecar_bom, materialize_sidecars,
};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn materializes_aube_lockfiles_as_pnpm_lock_and_cleans_up() {
    let root = repo_root();
    let lock = MiseLock::from_path(root.join("mise.lock")).expect("repo mise.lock should parse");

    let workspace = materialize_sidecars(root, Some(&lock), &SidecarOptions::default())
        .expect("materialization with digest verification should succeed");
    let ws_path = workspace.path().to_path_buf();
    assert!(ws_path.is_dir());

    let prettier_pnpm = ws_path.join("npm-prettier/3.9.10/pnpm-lock.yaml");
    let prettier_pkg = ws_path.join("npm-prettier/3.9.10/package.json");
    let commitlint_pnpm = ws_path.join("npm-commitlint-cli/21.2.4/pnpm-lock.yaml");
    let commitlint_pkg = ws_path.join("npm-commitlint-cli/21.2.4/package.json");

    assert!(prettier_pnpm.is_file(), "prettier pnpm-lock.yaml missing");
    assert!(prettier_pkg.is_file(), "prettier package.json missing");
    assert!(
        commitlint_pnpm.is_file(),
        "commitlint pnpm-lock.yaml missing"
    );
    assert!(commitlint_pkg.is_file(), "commitlint package.json missing");

    // Ensure .mise/locks/ was not modified in place
    assert!(
        !root
            .join(".mise/locks/npm-prettier/3.9.10/pnpm-lock.yaml")
            .exists()
    );

    workspace.close().expect("workspace cleanup should succeed");
    assert!(
        !ws_path.exists(),
        "temporary workspace should be removed after close"
    );
}

#[test]
fn digest_verification_detects_mismatch_and_can_be_skipped() {
    let tmp = TempDir::new().expect("create temp project");
    let sidecar_dir = tmp.path().join(".mise/locks/npm-prettier/3.9.10");
    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::copy(
        repo_root().join(".mise/locks/npm-prettier/3.9.10/aube-lock.yaml"),
        sidecar_dir.join("aube-lock.yaml"),
    )
    .unwrap();
    fs::copy(
        repo_root().join(".mise/locks/npm-prettier/3.9.10/package.json"),
        sidecar_dir.join("package.json"),
    )
    .unwrap();

    let lock_toml = r#"
lockfile_version = 3

[[tools."npm:prettier"]]
version = "3.9.10"
aube = { path = ".mise/locks/npm-prettier/3.9.10", digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000" }
backend = "npm:prettier"
"#;
    let lock = MiseLock::from_str(lock_toml).unwrap();

    let err = materialize_sidecars(tmp.path(), Some(&lock), &SidecarOptions::default())
        .expect_err("mismatched digest should fail when verification is enabled");
    assert!(matches!(err, SidecarError::DigestMismatch { .. }));

    let ws = materialize_sidecars(
        tmp.path(),
        Some(&lock),
        &SidecarOptions {
            verify_digests: false,
        },
    )
    .expect("materialization should succeed when digest verification is disabled");
    assert!(
        ws.path()
            .join("npm-prettier/3.9.10/pnpm-lock.yaml")
            .is_file()
    );
}

#[test]
fn preserves_native_uv_lockfiles_without_mise_lock() {
    let tmp = TempDir::new().expect("create temp project");
    let uv_dir = tmp.path().join(".mise/locks/pipx-black/25.1.0");
    fs::create_dir_all(&uv_dir).unwrap();
    fs::write(uv_dir.join("uv.lock"), "version = 1\n").unwrap();

    let ws = materialize_sidecars(tmp.path(), None, &SidecarOptions::default())
        .expect("discovery from .mise/locks/ should succeed");
    assert!(ws.path().join("pipx-black/25.1.0/uv.lock").is_file());
}

#[test]
fn extracts_direct_and_transitive_npm_components_and_dependencies_via_trivy() {
    let root = repo_root();
    let lock = MiseLock::from_path(root.join("mise.lock")).expect("repo mise.lock should parse");

    let bom = extract_sidecar_bom(root, Some(&lock), &SidecarOptions::default())
        .expect("trivy extraction should succeed");

    let npm_packages: Vec<_> = bom
        .package_components()
        .filter(|c| c.purl.as_deref().is_some_and(|p| p.starts_with("pkg:npm/")))
        .collect();

    assert!(
        npm_packages.len() > 2,
        "expected direct + transitive npm packages, got {}",
        npm_packages.len()
    );

    let prettier = bom
        .find_by_purl("pkg:npm/prettier@3.9.10")
        .expect("prettier component should be extracted");
    assert_eq!(prettier.version.as_deref(), Some("3.9.10"));

    let commitlint = bom
        .find_by_purl("pkg:npm/%40commitlint/cli@21.2.4")
        .expect("@commitlint/cli component should be extracted");
    let commitlint_ref = commitlint
        .bom_ref
        .as_deref()
        .expect("@commitlint/cli should have a bom-ref");

    let deps = bom
        .dependencies_for(commitlint_ref)
        .expect("@commitlint/cli should have a dependencies[] entry");
    assert!(
        !deps.is_empty(),
        "@commitlint/cli should have transitive dependsOn edges"
    );
}
