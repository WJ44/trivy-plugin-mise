use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer};
use thiserror::Error;

/// Supported `mise.lock` format version.
pub const SUPPORTED_LOCKFILE_VERSION: u64 = 3;

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

    #[error("unsupported mise.lock lockfile_version {0} (expected {SUPPORTED_LOCKFILE_VERSION})")]
    UnsupportedVersion(u64),

    #[error("invalid tool entry for `{tool}`: {reason}")]
    InvalidTool { tool: String, reason: String },
}

/// Parsed representation of a `mise.lock` file (`lockfile_version = 3`).
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
    /// Sidecar lockfile metadata keyed by manager name (for example `"aube"`, `"uv"`).
    pub sidecars: BTreeMap<String, SidecarLock>,
    /// Per-platform asset metadata keyed by platform triple (for example `"linux-x64"`).
    pub platforms: BTreeMap<String, PlatformAsset>,
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
    #[serde(default)]
    pub provenance: Option<String>,
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
        if raw.lockfile_version != SUPPORTED_LOCKFILE_VERSION {
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

    let mut platforms = BTreeMap::new();
    let mut sidecars = BTreeMap::new();

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
        } else if let Some(table) = value.as_table()
            && table.contains_key("path")
        {
            let sidecar: SidecarLock =
                value.try_into().map_err(|err| MiseLockError::InvalidTool {
                    tool: tool_name.to_string(),
                    reason: format!("invalid sidecar entry `{key}`: {err}"),
                })?;
            sidecars.insert(key, sidecar);
        }
    }

    Ok(LockedTool {
        name: tool_name.to_string(),
        version: raw.version,
        backend: raw.backend,
        specifiers: raw.specifiers,
        options: raw.options,
        sidecars,
        platforms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_lockfile_version() {
        let content = r#"
lockfile_version = 2
"#;
        let err = MiseLock::from_str(content).unwrap_err();
        assert!(matches!(err, MiseLockError::UnsupportedVersion(2)));
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
}
