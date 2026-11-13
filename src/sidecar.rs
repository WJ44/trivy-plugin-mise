use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use thiserror::Error;

use crate::mise_lock::MiseLock;

/// Native manifest and lockfile names preserved when materializing a sidecar directory.
const NATIVE_LOCK_AND_MANIFEST_FILES: &[&str] = &[
    "package.json",
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "uv.lock",
    "pyproject.toml",
    "Cargo.lock",
    "Cargo.toml",
];

/// Errors returned during sidecar discovery, digest verification, materialization, or Trivy extraction.
#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("invalid sidecar path `{path}`: {reason}")]
    InvalidSidecarPath { path: String, reason: String },

    #[error("missing sidecar directory `{path}`")]
    MissingSidecarDir { path: String },

    #[error("missing lockfile for digest verification in `{dir}`")]
    MissingLockfileForDigest { dir: String },

    #[error("invalid or unsupported digest `{digest}` for `{path}`")]
    InvalidDigest { path: String, digest: String },

    #[error("digest mismatch for `{path}`: expected {expected}, actual {actual}")]
    DigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("filesystem error at `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to invoke `trivy`: {0}")]
    TrivyInvocation(#[source] std::io::Error),

    #[error("`trivy fs` failed with status {status}: {stderr}")]
    TrivyFailed { status: String, stderr: String },

    #[error("failed to parse Trivy CycloneDX JSON output: {0}")]
    TrivyJson(#[from] serde_json::Error),
}

/// Options controlling sidecar discovery and materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarOptions {
    /// Verify SHA-256 digests when recorded in `mise.lock`.
    pub verify_digests: bool,
}

impl Default for SidecarOptions {
    fn default() -> Self {
        Self {
            verify_digests: true,
        }
    }
}

/// A discovered `.mise/locks/<tool>/<version>/` sidecar directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSidecar {
    pub tool_name: Option<String>,
    pub version: Option<String>,
    pub manager: Option<String>,
    pub source_dir: PathBuf,
    pub materialized_rel_dir: PathBuf,
    pub expected_digest: Option<String>,
}

/// Metadata for a single materialized sidecar directory inside [`MaterializedWorkspace`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedEntry {
    pub source_dir: PathBuf,
    pub dest_dir: PathBuf,
    pub materialized_files: Vec<PathBuf>,
}

/// Ephemeral workspace holding Trivy-compatible lockfiles, cleaned up on drop.
#[derive(Debug)]
pub struct MaterializedWorkspace {
    temp_dir: TempDir,
    entries: Vec<MaterializedEntry>,
}

impl MaterializedWorkspace {
    /// Returns the root path of the temporary materialized workspace.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Returns the materialized sidecar entries in deterministic order.
    #[must_use]
    pub fn entries(&self) -> &[MaterializedEntry] {
        &self.entries
    }

    /// Explicitly closes and removes the temporary workspace, surfacing any I/O error.
    pub fn close(self) -> std::io::Result<()> {
        self.temp_dir.close()
    }
}

/// Parsed CycloneDX BOM returned by `trivy fs --format cyclonedx`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrivyCycloneDxBom {
    #[serde(default)]
    pub bom_format: Option<String>,
    #[serde(default)]
    pub spec_version: Option<String>,
    #[serde(default)]
    pub components: Vec<TrivyBomComponent>,
    #[serde(default)]
    pub dependencies: Vec<TrivyBomDependency>,
}

impl TrivyCycloneDxBom {
    /// Iterates over package components that carry a PURL (excluding file/application root nodes without PURLs).
    pub fn package_components(&self) -> impl Iterator<Item = &TrivyBomComponent> {
        self.components.iter().filter(|c| c.purl.is_some())
    }

    /// Finds the first component matching an exact PURL string.
    #[must_use]
    pub fn find_by_purl(&self, purl: &str) -> Option<&TrivyBomComponent> {
        self.components
            .iter()
            .find(|c| c.purl.as_deref() == Some(purl))
    }

    /// Returns the `dependsOn` references recorded for `bom_ref`, if present.
    #[must_use]
    pub fn dependencies_for(&self, bom_ref: &str) -> Option<&[String]> {
        self.dependencies
            .iter()
            .find(|d| d.dep_ref == bom_ref)
            .map(|d| d.depends_on.as_slice())
    }
}

/// A component entry in Trivy's CycloneDX JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrivyBomComponent {
    #[serde(rename = "bom-ref", default)]
    pub bom_ref: Option<String>,
    #[serde(rename = "type")]
    pub component_type: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub purl: Option<String>,
    #[serde(default)]
    pub properties: Vec<TrivyBomProperty>,
}

/// A CycloneDX key-value property on a [`TrivyBomComponent`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TrivyBomProperty {
    pub name: String,
    pub value: String,
}

/// A dependency graph edge in Trivy's CycloneDX JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrivyBomDependency {
    #[serde(rename = "ref")]
    pub dep_ref: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Discovers sidecar lockfile directories referenced by `lock` and/or present under `<project_root>/.mise/locks/`.
pub fn discover_sidecars(
    project_root: &Path,
    lock: Option<&MiseLock>,
) -> Result<Vec<DiscoveredSidecar>, SidecarError> {
    let mut discovered = Vec::new();
    let mut seen_dirs = BTreeSet::new();

    if let Some(mise_lock) = lock {
        for tool in mise_lock.iter_tools() {
            for (manager, sidecar) in &tool.sidecars {
                let rel_path = validate_relative_path(&sidecar.path)?;
                let source_dir = project_root.join(&rel_path);
                if !source_dir.is_dir() {
                    return Err(SidecarError::MissingSidecarDir {
                        path: source_dir.display().to_string(),
                    });
                }

                let canonical =
                    fs::canonicalize(&source_dir).map_err(|source| SidecarError::Io {
                        path: source_dir.display().to_string(),
                        source,
                    })?;
                if !seen_dirs.insert(canonical) {
                    continue;
                }

                let materialized_rel_dir =
                    materialized_subdir(&rel_path, &tool.name, &tool.version);

                discovered.push(DiscoveredSidecar {
                    tool_name: Some(tool.name.clone()),
                    version: Some(tool.version.clone()),
                    manager: Some(manager.clone()),
                    source_dir,
                    materialized_rel_dir,
                    expected_digest: sidecar.digest.clone(),
                });
            }
        }
    }

    let locks_root = project_root.join(".mise").join("locks");
    if locks_root.is_dir() {
        for tool_dir in read_sorted_subdirs(&locks_root)? {
            let Some(tool_seg) = tool_dir.file_name() else {
                continue;
            };
            for ver_dir in read_sorted_subdirs(&tool_dir)? {
                let Some(ver_seg) = ver_dir.file_name() else {
                    continue;
                };
                let canonical = fs::canonicalize(&ver_dir).map_err(|source| SidecarError::Io {
                    path: ver_dir.display().to_string(),
                    source,
                })?;
                if !seen_dirs.insert(canonical) {
                    continue;
                }

                let version_str = ver_seg.to_string_lossy().into_owned();
                let materialized_rel_dir = PathBuf::from(tool_seg).join(ver_seg);

                discovered.push(DiscoveredSidecar {
                    tool_name: Some(tool_seg.to_string_lossy().into_owned()),
                    version: Some(version_str),
                    manager: None,
                    source_dir: ver_dir,
                    materialized_rel_dir,
                    expected_digest: None,
                });
            }
        }
    }

    Ok(discovered)
}

/// Verifies the SHA-256 digest of a discovered sidecar lockfile against `expected_digest`.
pub fn verify_sidecar_digest(
    sidecar: &DiscoveredSidecar,
    expected_digest: &str,
) -> Result<(), SidecarError> {
    let target_file = resolve_digest_target_file(sidecar)?;
    let (alg, expected_hex) =
        expected_digest
            .split_once(':')
            .ok_or_else(|| SidecarError::InvalidDigest {
                path: target_file.display().to_string(),
                digest: expected_digest.to_string(),
            })?;

    if !alg.trim().eq_ignore_ascii_case("sha256") {
        return Err(SidecarError::InvalidDigest {
            path: target_file.display().to_string(),
            digest: expected_digest.to_string(),
        });
    }

    let expected_hex = expected_hex.trim().to_ascii_lowercase();
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SidecarError::InvalidDigest {
            path: target_file.display().to_string(),
            digest: expected_digest.to_string(),
        });
    }

    let bytes = fs::read(&target_file).map_err(|source| SidecarError::Io {
        path: target_file.display().to_string(),
        source,
    })?;
    let actual_hex = format!("{:x}", Sha256::digest(&bytes));

    if actual_hex != expected_hex {
        return Err(SidecarError::DigestMismatch {
            path: target_file.display().to_string(),
            expected: format!("sha256:{expected_hex}"),
            actual: format!("sha256:{actual_hex}"),
        });
    }

    Ok(())
}

/// Discovers, optionally verifies, and materializes sidecar lockfiles into an ephemeral [`MaterializedWorkspace`].
pub fn materialize_sidecars(
    project_root: &Path,
    lock: Option<&MiseLock>,
    options: &SidecarOptions,
) -> Result<MaterializedWorkspace, SidecarError> {
    let discovered = discover_sidecars(project_root, lock)?;
    let temp_dir = tempfile::Builder::new()
        .prefix("trivy-plugin-mise-")
        .tempdir()
        .map_err(|source| SidecarError::Io {
            path: std::env::temp_dir().display().to_string(),
            source,
        })?;

    let mut entries = Vec::with_capacity(discovered.len());

    for sidecar in &discovered {
        if options.verify_digests
            && let Some(expected_digest) = &sidecar.expected_digest
        {
            verify_sidecar_digest(sidecar, expected_digest)?;
        }

        let dest_dir = temp_dir.path().join(&sidecar.materialized_rel_dir);
        fs::create_dir_all(&dest_dir).map_err(|source| SidecarError::Io {
            path: dest_dir.display().to_string(),
            source,
        })?;

        let mut materialized_files = Vec::new();

        for filename in NATIVE_LOCK_AND_MANIFEST_FILES {
            let src = sidecar.source_dir.join(filename);
            if src.is_file() {
                let dst = dest_dir.join(filename);
                copy_file(&src, &dst)?;
                materialized_files.push(dst);
            }
        }

        let aube_src = sidecar.source_dir.join("aube-lock.yaml");
        let pnpm_dst = dest_dir.join("pnpm-lock.yaml");
        if aube_src.is_file() && !pnpm_dst.exists() {
            copy_file(&aube_src, &pnpm_dst)?;
            materialized_files.push(pnpm_dst);
        }

        entries.push(MaterializedEntry {
            source_dir: sidecar.source_dir.clone(),
            dest_dir,
            materialized_files,
        });
    }

    Ok(MaterializedWorkspace { temp_dir, entries })
}

/// Runs `trivy fs --format cyclonedx --quiet` on a materialized workspace and parses the resulting BOM.
pub fn extract_cyclonedx_from_workspace(
    workspace: &MaterializedWorkspace,
) -> Result<TrivyCycloneDxBom, SidecarError> {
    let has_files = workspace
        .entries()
        .iter()
        .any(|entry| !entry.materialized_files.is_empty());
    if !has_files {
        return Ok(TrivyCycloneDxBom::default());
    }

    let output = Command::new("trivy")
        .args(["fs", "--format", "cyclonedx", "--quiet"])
        .arg(workspace.path())
        .output()
        .map_err(SidecarError::TrivyInvocation)?;

    if !output.status.success() {
        return Err(SidecarError::TrivyFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let bom: TrivyCycloneDxBom = serde_json::from_slice(&output.stdout)?;
    Ok(bom)
}

/// End-to-end helper: discovers sidecars, verifies digests, materializes into a temporary workspace,
/// extracts the CycloneDX BOM via Trivy, and cleans up the temporary workspace before returning.
pub fn extract_sidecar_bom(
    project_root: &Path,
    lock: Option<&MiseLock>,
    options: &SidecarOptions,
) -> Result<TrivyCycloneDxBom, SidecarError> {
    let workspace = materialize_sidecars(project_root, lock, options)?;
    let bom = extract_cyclonedx_from_workspace(&workspace)?;
    workspace.close().map_err(|source| SidecarError::Io {
        path: std::env::temp_dir().display().to_string(),
        source,
    })?;
    Ok(bom)
}

fn validate_relative_path(raw: &str) -> Result<PathBuf, SidecarError> {
    let path = Path::new(raw);
    if raw.trim().is_empty() || path.is_absolute() {
        return Err(SidecarError::InvalidSidecarPath {
            path: raw.to_string(),
            reason: "sidecar path must be a non-empty relative path".to_string(),
        });
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SidecarError::InvalidSidecarPath {
                    path: raw.to_string(),
                    reason: "sidecar path must not escape the project root".to_string(),
                });
            }
        }
    }

    Ok(path.to_path_buf())
}

fn materialized_subdir(rel_path: &Path, tool_name: &str, version: &str) -> PathBuf {
    let locks_prefix = Path::new(".mise").join("locks");
    if let Ok(stripped) = rel_path.strip_prefix(&locks_prefix)
        && !stripped.as_os_str().is_empty()
    {
        return stripped.to_path_buf();
    }

    let sanitized_tool = tool_name.replace(['/', ':', '@'], "-");
    PathBuf::from(sanitized_tool).join(version)
}

fn read_sorted_subdirs(dir: &Path) -> Result<Vec<PathBuf>, SidecarError> {
    let mut subdirs = Vec::new();
    let entries = fs::read_dir(dir).map_err(|source| SidecarError::Io {
        path: dir.display().to_string(),
        source,
    })?;

    for entry_res in entries {
        let entry = entry_res.map_err(|source| SidecarError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        }
    }

    subdirs.sort();
    Ok(subdirs)
}

fn resolve_digest_target_file(sidecar: &DiscoveredSidecar) -> Result<PathBuf, SidecarError> {
    let preferred: &[&str] = match sidecar.manager.as_deref() {
        Some("aube") => &["aube-lock.yaml"],
        Some("uv") => &["uv.lock"],
        _ => &[
            "aube-lock.yaml",
            "uv.lock",
            "pnpm-lock.yaml",
            "package-lock.json",
            "Cargo.lock",
        ],
    };

    for candidate in preferred {
        let path = sidecar.source_dir.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(SidecarError::MissingLockfileForDigest {
        dir: sidecar.source_dir.display().to_string(),
    })
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), SidecarError> {
    fs::copy(src, dst).map_err(|source| SidecarError::Io {
        path: format!("{} -> {}", src.display(), dst.display()),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal_in_sidecar_path() {
        let err = validate_relative_path("../outside").unwrap_err();
        assert!(matches!(err, SidecarError::InvalidSidecarPath { .. }));
    }

    #[test]
    fn computes_materialized_subdir_from_mise_locks_prefix() {
        let rel = Path::new(".mise/locks/npm-prettier/3.9.10");
        assert_eq!(
            materialized_subdir(rel, "npm:prettier", "3.9.10"),
            PathBuf::from("npm-prettier/3.9.10")
        );
    }
}
