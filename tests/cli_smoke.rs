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
fn plugin_yaml_satisfies_trivy_plugin_index_crawler_constraints() {
    let manifest_json = Command::new("yq")
        .args(["-o=json", "."])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin.yaml"))
        .output()
        .map(|out| {
            assert!(
                out.status.success(),
                "yq failed to parse plugin.yaml: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            serde_json::from_slice::<serde_json::Value>(&out.stdout)
                .expect("failed to deserialize yq JSON output for plugin.yaml")
        })
        .unwrap_or_else(|_| parse_plugin_yaml_fallback(PLUGIN_YAML));

    let name = manifest_json["name"]
        .as_str()
        .expect("plugin.yaml `name` must be a string");
    assert_eq!(name, "mise", "plugin.yaml `name` must be \"mise\"");
    assert!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "plugin.yaml `name` must be non-empty lowercase kebab-case"
    );

    let version = manifest_json["version"]
        .as_str()
        .expect("plugin.yaml `version` must be a string");
    assert!(
        !version.is_empty(),
        "plugin.yaml `version` must be non-empty"
    );
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "plugin.yaml `version` must match Cargo.toml package version"
    );

    let repository = manifest_json["repository"]
        .as_str()
        .expect("plugin.yaml `repository` must be a string");
    assert_eq!(
        repository, "github.com/WJ44/trivy-plugin-mise",
        "plugin.yaml `repository` must equal \"github.com/WJ44/trivy-plugin-mise\""
    );

    let maintainer = manifest_json["maintainer"]
        .as_str()
        .expect("plugin.yaml `maintainer` must be a string");
    assert!(
        !maintainer.is_empty() && maintainer.chars().all(|c| c.is_ascii_alphanumeric()),
        "plugin.yaml `maintainer` must be non-empty and strictly ASCII alphanumeric (`validate:\"required,alphanum\"`), got: {maintainer:?}"
    );

    let summary = manifest_json["summary"]
        .as_str()
        .expect("plugin.yaml `summary` must be a string");
    assert!(
        !summary.trim().is_empty(),
        "plugin.yaml `summary` must be non-empty"
    );

    let output = manifest_json["output"]
        .as_bool()
        .expect("plugin.yaml `output` must be a boolean");
    assert!(
        !output,
        "plugin.yaml `output` must be false for a CLI plugin"
    );

    let platforms = manifest_json["platforms"]
        .as_array()
        .expect("plugin.yaml `platforms` must be a non-empty sequence");
    assert!(
        !platforms.is_empty(),
        "plugin.yaml `platforms` must not be empty"
    );

    let expected_tag_segment = format!("/releases/download/v{version}/");
    for (idx, platform) in platforms.iter().enumerate() {
        let os = platform["selector"]["os"]
            .as_str()
            .unwrap_or_else(|| panic!("platforms[{idx}].selector.os must be a string"));
        assert!(
            matches!(os, "linux" | "darwin" | "windows"),
            "platforms[{idx}].selector.os has unexpected value: {os:?}"
        );

        let arch = platform["selector"]["arch"]
            .as_str()
            .unwrap_or_else(|| panic!("platforms[{idx}].selector.arch must be a string"));
        assert!(
            matches!(arch, "amd64" | "arm64"),
            "platforms[{idx}].selector.arch has unexpected value: {arch:?}"
        );

        let uri = platform["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("platforms[{idx}].uri must be a string"));
        assert!(
            uri.starts_with("https://github.com/WJ44/trivy-plugin-mise/releases/download/")
                && uri.contains(&expected_tag_segment),
            "platforms[{idx}].uri ({uri:?}) must match version tag v{version}"
        );

        let bin = platform["bin"]
            .as_str()
            .unwrap_or_else(|| panic!("platforms[{idx}].bin must be a string"));
        assert!(
            !bin.trim().is_empty() && bin.starts_with("./trivy-plugin-mise"),
            "platforms[{idx}].bin must point to `./trivy-plugin-mise[.exe]`, got: {bin:?}"
        );
    }
}

fn parse_plugin_yaml_fallback(yaml: &str) -> serde_json::Value {
    fn strip_comment_and_quotes(raw: &str) -> String {
        let trimmed = raw.trim();

        if let Some(rest) = trimmed.strip_prefix('"') {
            rest.split('"').next().unwrap_or("").to_string()
        } else {
            trimmed.split('#').next().unwrap_or("").trim().to_string()
        }
    }

    let (header, platforms_raw) = yaml
        .split_once("\nplatforms:\n")
        .expect("plugin.yaml must contain `platforms:` section");

    let mut name = String::new();
    let mut repository = String::new();
    let mut maintainer = String::new();
    let mut version = String::new();
    let mut summary = String::new();
    let mut output = None;
    let mut awaiting_summary_continuation = false;

    for line in header.lines() {
        if awaiting_summary_continuation {
            if line.starts_with("  ") {
                summary = strip_comment_and_quotes(line);
            }
            awaiting_summary_continuation = false;
            continue;
        }
        if let Some(val) = line.strip_prefix("name:") {
            name = strip_comment_and_quotes(val);
        } else if let Some(val) = line.strip_prefix("repository:") {
            repository = strip_comment_and_quotes(val);
        } else if let Some(val) = line.strip_prefix("maintainer:") {
            maintainer = strip_comment_and_quotes(val);
        } else if let Some(val) = line.strip_prefix("version:") {
            version = strip_comment_and_quotes(val);
        } else if let Some(val) = line.strip_prefix("summary:") {
            let inline = strip_comment_and_quotes(val);
            if inline.is_empty() {
                awaiting_summary_continuation = true;
            } else {
                summary = inline;
            }
        } else if let Some(val) = line.strip_prefix("output:") {
            output = match strip_comment_and_quotes(val).as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            };
        }
    }

    let mut platforms = Vec::new();
    let mut current_os = String::new();
    let mut current_arch = String::new();
    let mut current_uri = String::new();
    let mut current_bin = String::new();
    let mut in_item = false;

    for line in platforms_raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- selector:") {
            if in_item {
                platforms.push(serde_json::json!({
                    "selector": { "os": current_os, "arch": current_arch },
                    "uri": current_uri,
                    "bin": current_bin,
                }));
            }
            in_item = true;
            current_os.clear();
            current_arch.clear();
            current_uri.clear();
            current_bin.clear();
        } else if let Some(val) = trimmed.strip_prefix("os:") {
            current_os = strip_comment_and_quotes(val);
        } else if let Some(val) = trimmed.strip_prefix("arch:") {
            current_arch = strip_comment_and_quotes(val);
        } else if let Some(val) = trimmed.strip_prefix("uri:") {
            current_uri = strip_comment_and_quotes(val);
        } else if let Some(val) = trimmed.strip_prefix("bin:") {
            current_bin = strip_comment_and_quotes(val);
        }
    }
    if in_item {
        platforms.push(serde_json::json!({
            "selector": { "os": current_os, "arch": current_arch },
            "uri": current_uri,
            "bin": current_bin,
        }));
    }

    serde_json::json!({
        "name": name,
        "repository": repository,
        "maintainer": maintainer,
        "version": version,
        "summary": summary,
        "output": output,
        "platforms": platforms,
    })
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
