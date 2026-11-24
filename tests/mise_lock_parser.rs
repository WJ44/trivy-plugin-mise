use std::collections::BTreeMap;

use trivy_plugin_mise::component::{CycloneDxComponent, CycloneDxHash, normalize_lock};
use trivy_plugin_mise::mise_lock::{MiseLock, PackageProvenance, SUPPORTED_LOCKFILE_VERSIONS};
use trivy_plugin_mise::purl::tool_to_purl;

const FIXTURE_LOCK: &str = include_str!("fixtures/mise.lock");
const FIXTURE_V4_LOCK: &str = include_str!("fixtures/mise.v4.lock");
const REPO_LOCK: &str = include_str!("../mise.lock");
const HARNESS_LOCK: &str = include_str!("../.config/mise/mise.lock");

fn prop<'a>(component: &'a CycloneDxComponent, key: &str) -> Option<&'a str> {
    component
        .properties
        .iter()
        .find(|p| p.name == key)
        .map(|p| p.value.as_str())
}

#[test]
fn parses_fixture_and_verifies_all_tools_and_purls() {
    let lock = MiseLock::from_str(FIXTURE_LOCK).expect("fixture mise.lock should parse");
    assert_eq!(lock.lockfile_version, 3);
    assert!(SUPPORTED_LOCKFILE_VERSIONS.contains(&lock.lockfile_version));

    let expected_purls: BTreeMap<&str, &str> = BTreeMap::from([
        ("cargo:ripgrep", "pkg:cargo/ripgrep@14.1.1"),
        ("github:cli/cli", "pkg:github/cli/cli@2.67.0"),
        ("lefthook", "pkg:github/evilmartians/lefthook@2.2.2"),
        (
            "node",
            "pkg:generic/node@26.13.1?download_url=https%3A%2F%2Fnodejs.org%2Fdist%2Fv26.13.1%2Fnode-v26.13.1-linux-x64.tar.gz",
        ),
        ("npm:@commitlint/cli", "pkg:npm/%40commitlint/cli@21.2.4"),
        ("npm:prettier", "pkg:npm/prettier@3.9.10"),
        ("pipx:black", "pkg:pypi/black@25.1.0"),
        ("ruff", "pkg:github/astral-sh/ruff@0.17.4"),
        ("shellcheck", "pkg:github/koalaman/shellcheck@0.11.0"),
        ("shfmt", "pkg:github/mvdan/sh@3.14.1"),
        ("tombi", "pkg:github/tombi-toml/tombi@1.11.2"),
        ("trivy", "pkg:github/aquasecurity/trivy@0.76.2"),
        ("ubi:sharkdp/bat", "pkg:github/sharkdp/bat@0.25.0"),
    ]);

    for (name, expected_purl) in &expected_purls {
        let tool = &lock.tools[*name][0];
        let actual_purl = tool_to_purl(tool, Some("linux-x64"))
            .expect("PURL mapping should succeed")
            .to_string();
        assert_eq!(actual_purl, *expected_purl, "PURL mismatch for {name}");
    }

    // Verify extracted platform fields on lefthook
    let lefthook = &lock.tools["lefthook"][0];
    assert_eq!(lefthook.version, "2.2.2");
    assert_eq!(
        lefthook.backend.as_deref(),
        Some("aqua:evilmartians/lefthook")
    );
    let lh_linux = &lefthook.platforms["linux-x64"];
    assert_eq!(
        lh_linux.checksum.as_deref(),
        Some("sha256:2b8d7fd52771f26d7a50260ef10cd6fe704d66f60102e566362b84da09884ab7")
    );
    assert_eq!(
        lh_linux.url.as_deref(),
        Some(
            "https://github.com/evilmartians/lefthook/releases/download/v2.2.2/lefthook_2.2.2_Linux_x86_64.gz"
        )
    );
    assert_eq!(
        lh_linux.url_api.as_deref(),
        Some("https://api.github.com/repos/evilmartians/lefthook/releases/assets/636518865")
    );
    assert_eq!(lh_linux.provenance.as_deref(), Some("github-attestations"));

    // Verify checksum_source on shellcheck
    let shellcheck = &lock.tools["shellcheck"][0];
    assert_eq!(
        shellcheck.platforms["linux-x64"].checksum_source.as_deref(),
        Some("mise-versions")
    );

    // Verify sidecar lock paths (aube & uv)
    let prettier = &lock.tools["npm:prettier"][0];
    assert_eq!(
        prettier.sidecars["aube"].path,
        ".mise/locks/npm-prettier/3.9.10"
    );
    assert_eq!(
        prettier.sidecars["aube"].digest.as_deref(),
        Some("sha256:0a52cf3e98ebe971f9f0c338c08e668756d290a8d14fe3fd308251084fba1522")
    );

    let commitlint = &lock.tools["npm:@commitlint/cli"][0];
    assert_eq!(
        commitlint.sidecars["aube"].path,
        ".mise/locks/npm-commitlint-cli/21.2.4"
    );

    let black = &lock.tools["pipx:black"][0];
    assert_eq!(black.sidecars["uv"].path, ".mise/locks/pipx-black/25.1.0");
}

#[test]
fn normalizes_fixture_to_cyclonedx_components() {
    let lock = MiseLock::from_str(FIXTURE_LOCK).expect("fixture mise.lock should parse");
    let components = normalize_lock(&lock, Some("linux-x64")).expect("normalize should succeed");
    assert_eq!(components.len(), 13);

    let trivy = components
        .iter()
        .find(|c| c.name == "trivy")
        .expect("trivy component");
    assert_eq!(trivy.purl, "pkg:github/aquasecurity/trivy@0.76.2");
    assert_eq!(
        trivy.hashes,
        vec![CycloneDxHash {
            alg: "SHA-256".to_string(),
            content: "bc0fc726e017b8927a62b2d3531d7a84ee01769daf5c99ed866dced54089fdbf".to_string(),
        }]
    );
    assert_eq!(prop(trivy, "mise:backend"), Some("aqua:aquasecurity/trivy"));
    assert_eq!(prop(trivy, "mise:provenance"), Some("cosign"));

    let prettier = components
        .iter()
        .find(|c| c.name == "prettier")
        .expect("prettier component");
    assert_eq!(prettier.purl, "pkg:npm/prettier@3.9.10");
    assert_eq!(
        prop(prettier, "mise:aube:path"),
        Some(".mise/locks/npm-prettier/3.9.10")
    );

    let lefthook = components
        .iter()
        .find(|c| c.name == "lefthook")
        .expect("lefthook component");
    assert_eq!(
        prop(lefthook, "mise:provenance"),
        Some("github-attestations")
    );
}

#[test]
fn parses_and_normalizes_repository_mise_lock() {
    let lock = MiseLock::from_str(REPO_LOCK).expect("repo mise.lock should parse");
    let components = normalize_lock(&lock, None).expect("normalize should succeed");
    assert!(!components.is_empty());

    let rust = components
        .iter()
        .find(|c| c.name == "rust")
        .expect("rust in repo mise.lock");
    assert_eq!(prop(rust, "mise:backend"), Some("core:rust"));

    let harness_lock =
        MiseLock::from_str(HARNESS_LOCK).expect(".config/mise/mise.lock should parse");
    let harness_components =
        normalize_lock(&harness_lock, None).expect("harness normalize should succeed");
    let trivy = harness_components
        .iter()
        .find(|c| c.name == "trivy")
        .expect("trivy in .config/mise/mise.lock");
    assert_eq!(prop(trivy, "mise:provenance"), Some("cosign"));
}

#[test]
fn parses_v4_fixture_and_normalizes_components() {
    let lock = MiseLock::from_str(FIXTURE_V4_LOCK).expect("v4 fixture mise.lock should parse");
    assert_eq!(lock.lockfile_version, 4);
    assert!(SUPPORTED_LOCKFILE_VERSIONS.contains(&lock.lockfile_version));

    let bundle = &lock.tools["npm:@sigstore/bundle"][0];
    assert_eq!(
        bundle.provenance,
        Some(PackageProvenance {
            repository: "github.com/sigstore/sigstore-js".to_string(),
        })
    );
    assert!(
        bundle.sidecars.is_empty(),
        "tool-level provenance must not be misclassified as a sidecar"
    );

    let prettier = &lock.tools["npm:prettier"][0];
    assert_eq!(
        prettier.provenance,
        Some(PackageProvenance {
            repository: "github.com/prettier/prettier".to_string(),
        })
    );
    assert_eq!(
        prettier.sidecars["aube"].path,
        ".mise/locks/npm-prettier/3.10.1"
    );
    assert!(!prettier.sidecars.contains_key("provenance"));

    let lefthook = &lock.tools["lefthook"][0];
    assert_eq!(
        lefthook.extra["unsupported_platforms"].as_array().unwrap()[0].as_str(),
        Some("windows-arm64")
    );

    let hk = &lock.tools["hk"][0];
    assert_eq!(
        hk.platforms["linux-x64"].checksum.as_deref(),
        Some("sha256:449b83c35ac3a460f80a4c9a41ad99430add8cf6caa6ceadc27e810feac3647d")
    );

    let slsatool = &lock.tools["slsatool"][0];
    assert_eq!(
        slsatool.platforms["linux-x64"].provenance.as_deref(),
        Some("slsa")
    );

    let components = normalize_lock(&lock, Some("linux-x64")).expect("normalize v4 should succeed");
    assert_eq!(components.len(), 6);

    let bundle_comp = components
        .iter()
        .find(|c| c.purl == "pkg:npm/%40sigstore/bundle@3.1.0")
        .expect("@sigstore/bundle component");
    assert_eq!(
        prop(bundle_comp, "mise:provenance:repository"),
        Some("github.com/sigstore/sigstore-js")
    );

    let prettier_comp = components
        .iter()
        .find(|c| c.purl == "pkg:npm/prettier@3.10.1")
        .expect("prettier component");
    assert_eq!(
        prop(prettier_comp, "mise:provenance:repository"),
        Some("github.com/prettier/prettier")
    );
    assert_eq!(
        prop(prettier_comp, "mise:aube:path"),
        Some(".mise/locks/npm-prettier/3.10.1")
    );

    let slsa_comp = components
        .iter()
        .find(|c| c.name == "slsatool")
        .expect("slsatool component");
    assert_eq!(prop(slsa_comp, "mise:provenance"), Some("slsa"));
}
