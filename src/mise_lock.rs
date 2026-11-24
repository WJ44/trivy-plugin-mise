use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer};
use thiserror::Error;

/// Supported `mise.lock` format versions.
pub const SUPPORTED_LOCKFILE_VERSIONS: &[u64] = &[3, 4];

/// Latest supported `mise.lock` format version.
///
/// Prefer [`SUPPORTED_LOCKFILE_VERSIONS`] when checking whether a lockfile
/// version is supported.
pub const SUPPORTED_LOCKFILE_VERSION: u64 = 4;

fn format_supported_versions() -> String {
    SUPPORTED_LOCKFILE_VERSIONS
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Errors returned while reading or parsing a `mise.lock` file.
#[derive(Debug, Error)]
pub enum MiseLockError {
    #[error("failed to read mise.lock from {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse mise.lock TOML: {0}")]
    Parse(#[from] toml::de::Error),

    #[error(
        "unsupported mise.lock lockfile_version {0} (supported versions: {supported})",
        supported = format_supported_versions()
    )]
    UnsupportedVersion(u64),

    #[error("invalid tool entry for `{tool}`: {reason}")]
    InvalidTool { tool: String, reason: String },
}

/// Parsed representation of a `mise.lock` file (`lockfile_version = 3` or `4`).
#[derive(Debug, Clone, PartialEq)]
pub struct MiseLock {
    pub lockfile_version: u64,
    pub tools: BTreeMap<String, Vec<LockedTool>>,
}

/// A single locked tool entry under `[[tools.<name>]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct LockedTool {
    /// Tool key from `[tools.<name>]` (for example `"lefthook"` or `"npm:prettier"`).
    pub name: String,
    /// Locked version string.
    pub version: String,
    /// Mise backend string (for example `"aqua:evilmartians/lefthook"` or `"core:node"`).
    pub backend: Option<String>,
    /// Requested version specifiers.
    pub specifiers: Vec<String>,
    /// Tool-specific options table (`[tools.<name>.options]`).
    pub options: BTreeMap<String, toml::Value>,
    /// Package-level provenance pin (`provenance = { repository = "..." }`, added in `lockfile_version = 4`).
    pub provenance: Option<PackageProvenance>,
    /// Sidecar lockfile metadata keyed by manager name (for example `"aube"`, `"uv"`).
    pub sidecars: BTreeMap<String, SidecarLock>,
    /// Per-platform asset metadata keyed by platform triple (for example `"linux-x64"`).
    pub platforms: BTreeMap<String, PlatformAsset>,
    /// Additional unrecognized top-level tool fields preserved for forward compatibility.
    pub extra: BTreeMap<String, toml::Value>,
}

/// Package-level provenance pin from a `lockfile_version = 4` tool entry
/// (for example `provenance = { repository = "github.com/sigstore/sigstore-js" }`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PackageProvenance {
    pub repository: String,
}

impl LockedTool {
    /// Returns the explicit `backend` if present, otherwise falls back to the tool key `name`.
    #[must_use]
    pub fn effective_backend(&self) -> &str {
        self.backend.as_deref().unwrap_or(&self.name)
    }
}

/// Sidecar lockfile reference embedded on a tool entry (for example `aube = { path = ..., digest = ... }`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SidecarLock {
    pub path: String,
    #[serde(default)]
    pub digest: Option<String>,
}

/// Per-platform artifact metadata from `[tools.<name>."platforms.<os>-<arch>"]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct PlatformAsset {
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub checksum_source: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub url_api: Option<String>,
    #[serde(default, deserialize_with = "deserialize_platform_provenance")]
    pub provenance: Option<String>,
}

fn deserialize_platform_provenance<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<toml::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };

    match value {
        toml::Value::String(s) => Ok(Some(s)),
        toml::Value::Table(table) if table.contains_key("slsa") => Ok(Some("slsa".to_string())),
        toml::Value::Table(table) => {
            let keys: Vec<&str> = table.keys().map(String::as_str).collect();
            Err(serde::de::Error::custom(format!(
                "unrecognized platform provenance table keys: {keys:?}"
            )))
        }
        other => Err(serde::de::Error::custom(format!(
            "expected string or `{{ slsa = {{ ... }} }}` table for platform provenance, got {}",
            other.type_str()
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct RawMiseLock {
    lockfile_version: u64,
    #[serde(default)]
    tools: BTreeMap<String, OneOrMany<RawLockedTool>>,
}

#[derive(Debug, Deserialize)]
struct RawLockedTool {
    version: String,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    specifiers: Vec<String>,
    #[serde(default)]
    options: BTreeMap<String, toml::Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug)]
struct OneOrMany<T>(Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for OneOrMany<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<T> {
            Many(Vec<T>),
            One(T),
        }

        match Helper::<T>::deserialize(deserializer)? {
            Helper::Many(items) => Ok(Self(items)),
            Helper::One(item) => Ok(Self(vec![item])),
        }
    }
}

impl MiseLock {
    /// Reads and parses a `mise.lock` file from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, MiseLockError> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref).map_err(|source| MiseLockError::Io {
            path: path_ref.display().to_string(),
            source,
        })?;
        Self::from_str(&content)
    }

    /// Parses `mise.lock` TOML content.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self, MiseLockError> {
        let raw: RawMiseLock = toml::from_str(content)?;
        if !SUPPORTED_LOCKFILE_VERSIONS.contains(&raw.lockfile_version) {
            return Err(MiseLockError::UnsupportedVersion(raw.lockfile_version));
        }

        let mut tools = BTreeMap::new();
        for (tool_name, OneOrMany(entries)) in raw.tools {
            let mut parsed_entries = Vec::with_capacity(entries.len());
            for raw_entry in entries {
                parsed_entries.push(parse_locked_tool(&tool_name, raw_entry)?);
            }
            tools.insert(tool_name, parsed_entries);
        }

        Ok(Self {
            lockfile_version: raw.lockfile_version,
            tools,
        })
    }

    /// Iterates over all locked tool entries in deterministic `(tool_name, index)` order.
    pub fn iter_tools(&self) -> impl Iterator<Item = &LockedTool> {
        self.tools.values().flat_map(|entries| entries.iter())
    }
}

fn parse_locked_tool(tool_name: &str, raw: RawLockedTool) -> Result<LockedTool, MiseLockError> {
    if raw.version.trim().is_empty() {
        return Err(MiseLockError::InvalidTool {
            tool: tool_name.to_string(),
            reason: "version must not be empty".to_string(),
        });
    }

    let mut provenance = None;
    let mut platforms = BTreeMap::new();
    let mut sidecars = BTreeMap::new();
    let mut extra = BTreeMap::new();

    for (key, value) in raw.extra {
        if let Some(platform) = key.strip_prefix("platforms.") {
            if platform.is_empty() {
                return Err(MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: "platform key `platforms.` is missing a platform name".to_string(),
                });
            }
            let asset: PlatformAsset =
                value.try_into().map_err(|err| MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: format!("invalid platform entry `{key}`: {err}"),
                })?;
            platforms.insert(platform.to_string(), asset);
        } else if key == "provenance" {
            let parsed: PackageProvenance =
                value.try_into().map_err(|err| MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: format!("invalid provenance entry: {err}"),
                })?;
            if parsed.repository.trim().is_empty() {
                return Err(MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: "provenance.repository must not be empty".to_string(),
                });
            }
            provenance = Some(parsed);
        } else if let Some(table) = value.as_table()
            && table.contains_key("path")
        {
            let sidecar: SidecarLock =
                value.try_into().map_err(|err| MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: format!("invalid sidecar entry `{key}`: {err}"),
                })?;
            sidecars.insert(key, sidecar);
        } else {
            extra.insert(key, value);
        }
    }

    Ok(LockedTool {
        name: tool_name.to_string(),
        version: raw.version,
        backend: raw.backend,
        specifiers: raw.specifiers,
        options: raw.options,
        provenance,
        sidecars,
        platforms,
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_lockfile_version() {
        for version in [0_u64, 2, 5] {
            let content = format!("lockfile_version = {version}\n");
            let err = MiseLock::from_str(&content).unwrap_err();
            assert!(
                matches!(err, MiseLockError::UnsupportedVersion(v) if v == version),
                "expected UnsupportedVersion({version}), got {err:?}"
            );
            assert_eq!(
                err.to_string(),
                format!(
                    "unsupported mise.lock lockfile_version {version} (supported versions: 3, 4)"
                )
            );
        }
    }

    #[test]
    fn accepts_lockfile_version_3_and_4() {
        for version in [3_u64, 4] {
            let content = format!(
                r#"
lockfile_version = {version}

[[tools.node]]
version = "24.21.0"
backend = "core:node"
"#
            );
            let lock = MiseLock::from_str(&content).unwrap();
            assert_eq!(lock.lockfile_version, version);
            assert_eq!(lock.tools["node"][0].version, "24.21.0");
        }
    }

    #[test]
    fn rejects_empty_version() {
        let content = r#"
lockfile_version = 3

[[tools.node]]
version = ""
backend = "core:node"
"#;
        let err = MiseLock::from_str(content).unwrap_err();
        assert!(matches!(err, MiseLockError::InvalidTool { .. }));
    }

    #[test]
    fn parses_sidecars_options_and_platforms() {
        let content = r#"
lockfile_version = 3

[[tools."npm:@commitlint/cli"]]
version = "21.2.4"
aube = { path = ".mise/locks/npm-commitlint-cli/21.2.4", digest = "sha256:abc" }
backend = "npm:@commitlint/cli"
specifiers = ["21.2.4"]

[[tools.uvtool]]
version = "1.0.0"
uv = { path = ".mise/locks/uvtool/1.0.0" }
backend = "pipx:uvtool"

[[tools.rust]]
version = "1.100.0"
backend = "core:rust"
specifiers = ["1.100.0"]

[tools.rust.options]
components = "clippy,rustfmt"
"#;
        let lock = MiseLock::from_str(content).unwrap();
        let commitlint = &lock.tools["npm:@commitlint/cli"][0];
        assert_eq!(
            commitlint.sidecars["aube"],
            SidecarLock {
                path: ".mise/locks/npm-commitlint-cli/21.2.4".to_string(),
                digest: Some("sha256:abc".to_string()),
            }
        );

        let uvtool = &lock.tools["uvtool"][0];
        assert_eq!(
            uvtool.sidecars["uv"],
            SidecarLock {
                path: ".mise/locks/uvtool/1.0.0".to_string(),
                digest: None,
            }
        );

        let rust = &lock.tools["rust"][0];
        assert_eq!(rust.options["components"].as_str(), Some("clippy,rustfmt"));
    }

    #[test]
    fn parses_v4_tool_provenance_without_sidecar_misclassification() {
        let content = r#"
lockfile_version = 4

[[tools."npm:@sigstore/bundle"]]
version = "3.1.0"
backend = "npm:@sigstore/bundle"
provenance = { repository = "github.com/sigstore/sigstore-js" }

[[tools."npm:prettier"]]
version = "3.10.1"
aube = { path = ".mise/locks/npm-prettier/3.10.1", digest = "sha256:abc" }
backend = "npm:prettier"
provenance = { repository = "github.com/prettier/prettier" }
"#;
        let lock = MiseLock::from_str(content).unwrap();
        assert_eq!(lock.lockfile_version, 4);

        let bundle = &lock.tools["npm:@sigstore/bundle"][0];
        assert_eq!(
            bundle.provenance,
            Some(PackageProvenance {
                repository: "github.com/sigstore/sigstore-js".to_string(),
            })
        );
        assert!(bundle.sidecars.is_empty());
        assert!(bundle.platforms.is_empty());
        assert!(!bundle.extra.contains_key("provenance"));

        let prettier = &lock.tools["npm:prettier"][0];
        assert_eq!(
            prettier.provenance,
            Some(PackageProvenance {
                repository: "github.com/prettier/prettier".to_string(),
            })
        );
        assert_eq!(
            prettier.sidecars["aube"],
            SidecarLock {
                path: ".mise/locks/npm-prettier/3.10.1".to_string(),
                digest: Some("sha256:abc".to_string()),
            }
        );
        assert!(!prettier.sidecars.contains_key("provenance"));
    }

    #[test]
    fn rejects_invalid_v4_tool_provenance() {
        let cases = [
            r#"
lockfile_version = 4

[[tools."npm:@sigstore/bundle"]]
version = "3.1.0"
provenance = "github-attestations"
"#,
            r#"
lockfile_version = 4

[[tools."npm:@sigstore/bundle"]]
version = "3.1.0"
provenance = {}
"#,
            r#"
lockfile_version = 4

[[tools."npm:@sigstore/bundle"]]
version = "3.1.0"
provenance = { repository = "   " }
"#,
        ];

        for content in cases {
            let err = MiseLock::from_str(content).unwrap_err();
            assert!(
                matches!(err, MiseLockError::InvalidTool { .. }),
                "expected InvalidTool, got {err:?}"
            );
        }
    }

    #[test]
    fn parses_v4_packslip_slsa_and_unsupported_platforms() {
        let content = r#"
lockfile_version = 4

[[tools.hk]]
version = "2.5.0"
backend = "packslip:github.com/jdx/hk"
specifiers = ["latest"]
unsupported_platforms = ["windows-arm64"]

[tools.hk."platforms.linux-x64"]
checksum = "sha256:449b83c35ac3a460f80a4c9a41ad99430add8cf6caa6ceadc27e810feac3647d"
url = "https://github.com/jdx/hk/releases/download/v2.5.0/hk-x86_64-unknown-linux-gnu.tar.gz"
signer = "sigstore-oidc:https://github.com/jdx/hk/.github/workflows/release.yml"
attested_by = "repackager"
repository_ids = { repository = "922514152" }

[[tools.slsatool]]
version = "1.0.0"
backend = "github:example/slsatool"

[tools.slsatool."platforms.linux-x64"]
checksum = "sha256:3333333333333333333333333333333333333333333333333333333333333333"
url = "https://github.com/example/slsatool/releases/download/v1.0.0/slsatool.tar.gz"
provenance = { slsa = { url = "https://github.com/example/slsatool/releases/download/v1.0.0/multiple.intoto.jsonl" } }
"#;
        let lock = MiseLock::from_str(content).unwrap();

        let hk = &lock.tools["hk"][0];
        assert!(hk.sidecars.is_empty());
        assert!(hk.provenance.is_none());
        assert_eq!(
            hk.extra["unsupported_platforms"].as_array().unwrap()[0].as_str(),
            Some("windows-arm64")
        );
        assert_eq!(
            hk.platforms["linux-x64"].checksum.as_deref(),
            Some("sha256:449b83c35ac3a460f80a4c9a41ad99430add8cf6caa6ceadc27e810feac3647d")
        );

        let slsatool = &lock.tools["slsatool"][0];
        assert_eq!(
            slsatool.platforms["linux-x64"].provenance.as_deref(),
            Some("slsa")
        );
    }

    #[test]
    fn preserves_unrecognized_tool_fields_in_extra() {
        let content = r#"
lockfile_version = 3

[[tools.node]]
version = "26.13.1"
backend = "core:node"
install_env = { NODE_OPTIONS = "--max-old-space-size=4096" }
source = "mise.toml"
"#;
        let lock = MiseLock::from_str(content).unwrap();
        let node = &lock.tools["node"][0];
        assert_eq!(node.extra["source"].as_str(), Some("mise.toml"));
        assert_eq!(
            node.extra["install_env"]["NODE_OPTIONS"].as_str(),
            Some("--max-old-space-size=4096")
        );
    }
}
