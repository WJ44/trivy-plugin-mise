use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use thiserror::Error;

use crate::mise_config::{
    ResolvedMiseProject, discover_standard_sidecar_dirs, non_empty_parent,
    sidecar_dir_matches_active_tools,
};
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

/// Discovers sidecar lockfile directories referenced by `lock` and/or present under standard sidecar lock directories.
pub fn discover_sidecars(
    project_root: &Path,
    lock: Option<&MiseLock>,
) -> Result<Vec<DiscoveredSidecar>, SidecarError> {
    let mut discovered = Vec::new();
    let mut seen_dirs = BTreeSet::new();
    let mut used_materialized_dirs = HashSet::new();

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
                ensure_within_root(project_root, &canonical, Path::new(&sidecar.path))?;
                if !seen_dirs.insert(canonical) {
                    continue;
                }

                let materialized_rel_dir = unique_materialized_dir(
                    materialized_subdir(&rel_path, &tool.name, &tool.version),
                    &mut used_materialized_dirs,
                );

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

    let sidecar_roots = discover_standard_sidecar_dirs(project_root);
    walk_sidecar_roots(
        project_root,
        &sidecar_roots,
        None,
        &BTreeSet::new(),
        &mut seen_dirs,
        &mut used_materialized_dirs,
        &mut discovered,
    )?;

    Ok(discovered)
}

/// Discovers sidecar lockfile directories for a [`ResolvedMiseProject`], honoring layered
/// lockfile-relative sidecar paths and excluding omitted or superseded tools.
pub fn discover_project_sidecars(
    project_root: &Path,
    project: &ResolvedMiseProject,
) -> Result<Vec<DiscoveredSidecar>, SidecarError> {
    let mut discovered = Vec::new();
    let mut seen_dirs = BTreeSet::new();
    let mut used_materialized_dirs = HashSet::new();

    let canonical_project_root = fs::canonicalize(project_root).ok();

    let mut excluded_dirs = BTreeSet::new();
    for excluded in &project.excluded_sidecars {
        let Ok(rel_path) = validate_relative_path(&excluded.sidecar_path) else {
            continue;
        };
        let lock_dir = non_empty_parent(&excluded.lock_path);
        let (candidate, _) = resolve_sidecar_source_dir(project_root, lock_dir, &rel_path);
        if candidate.is_dir()
            && let Ok(canonical) = fs::canonicalize(&candidate)
        {
            excluded_dirs.insert(canonical);
        }
    }

    for resolved in &project.tools {
        let lock_dir = non_empty_parent(&resolved.lock_path);
        let lock_dir_in_project = if let (Some(root_canon), Ok(lock_canon)) =
            (&canonical_project_root, fs::canonicalize(lock_dir))
        {
            lock_canon.starts_with(root_canon)
        } else {
            false
        };

        for (manager, sidecar) in &resolved.tool.sidecars {
            let rel_path = validate_relative_path(&sidecar.path)?;
            let (source_dir, resolved_from_project_root) =
                resolve_sidecar_source_dir(project_root, lock_dir, &rel_path);
            if !source_dir.is_dir() {
                return Err(SidecarError::MissingSidecarDir {
                    path: source_dir.display().to_string(),
                });
            }

            let canonical = fs::canonicalize(&source_dir).map_err(|source| SidecarError::Io {
                path: source_dir.display().to_string(),
                source,
            })?;
            let allowed_root = if resolved_from_project_root || lock_dir_in_project {
                project_root
            } else {
                lock_dir
            };
            ensure_within_root(allowed_root, &canonical, Path::new(&sidecar.path))?;
            if !seen_dirs.insert(canonical) {
                continue;
            }

            let materialized_rel_dir = unique_materialized_dir(
                materialized_subdir(&rel_path, &resolved.tool.name, &resolved.tool.version),
                &mut used_materialized_dirs,
            );

            discovered.push(DiscoveredSidecar {
                tool_name: Some(resolved.tool.name.clone()),
                version: Some(resolved.tool.version.clone()),
                manager: Some(manager.clone()),
                source_dir,
                materialized_rel_dir,
                expected_digest: sidecar.digest.clone(),
            });
        }
    }

    walk_sidecar_roots(
        project_root,
        &project.sidecar_lock_dirs,
        project.active_tools.as_ref(),
        &excluded_dirs,
        &mut seen_dirs,
        &mut used_materialized_dirs,
        &mut discovered,
    )?;

    Ok(discovered)
}

/// Resolves a sidecar path relative to `lock_dir` first, falling back to `project_root`.
/// Returns `(resolved_path, resolved_from_project_root)`.
fn resolve_sidecar_source_dir(
    project_root: &Path,
    lock_dir: &Path,
    rel_path: &Path,
) -> (PathBuf, bool) {
    let under_lock = lock_dir.join(rel_path);
    if under_lock.is_dir() || lock_dir == project_root {
        return (under_lock, lock_dir == project_root);
    }
    let under_root = project_root.join(rel_path);
    if under_root.is_dir() {
        (under_root, true)
    } else {
        (under_lock, false)
    }
}

fn walk_sidecar_roots(
    project_root: &Path,
    sidecar_roots: &[PathBuf],
    active_tools: Option<&BTreeSet<String>>,
    excluded_dirs: &BTreeSet<PathBuf>,
    seen_dirs: &mut BTreeSet<PathBuf>,
    used_materialized_dirs: &mut HashSet<PathBuf>,
    discovered: &mut Vec<DiscoveredSidecar>,
) -> Result<(), SidecarError> {
    for locks_root in sidecar_roots {
        if !locks_root.is_dir() {
            continue;
        }
        for tool_dir in read_sorted_subdirs(locks_root)? {
            let Some(tool_seg) = tool_dir.file_name() else {
                continue;
            };
            let tool_seg_str = tool_seg.to_string_lossy();
            if let Some(active) = active_tools
                && !sidecar_dir_matches_active_tools(&tool_seg_str, active)
            {
                continue;
            }
            for ver_dir in read_sorted_subdirs(&tool_dir)? {
                let Some(ver_seg) = ver_dir.file_name() else {
                    continue;
                };
                let canonical = fs::canonicalize(&ver_dir).map_err(|source| SidecarError::Io {
                    path: ver_dir.display().to_string(),
                    source,
                })?;
                ensure_within_root(project_root, &canonical, &ver_dir)?;
                if excluded_dirs.contains(&canonical) || !seen_dirs.insert(canonical) {
                    continue;
                }

                let version_str = ver_seg.to_string_lossy().into_owned();
                let materialized_rel_dir = unique_materialized_dir(
                    PathBuf::from(tool_seg).join(ver_seg),
                    used_materialized_dirs,
                );

                discovered.push(DiscoveredSidecar {
                    tool_name: Some(tool_seg_str.clone().into_owned()),
                    version: Some(version_str),
                    manager: None,
                    source_dir: ver_dir,
                    materialized_rel_dir,
                    expected_digest: None,
                });
            }
        }
    }
    Ok(())
}

fn unique_materialized_dir(base: PathBuf, used: &mut HashSet<PathBuf>) -> PathBuf {
    if used.insert(base.clone()) {
        return base;
    }
    let mut suffix = 2usize;
    loop {
        let candidate = PathBuf::from(format!("{}-{suffix}", base.display()));
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
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
    materialize_discovered_sidecars(&discovered, options)
}

/// Discovers, optionally verifies, and materializes sidecar lockfiles for a [`ResolvedMiseProject`].
pub fn materialize_project_sidecars(
    project_root: &Path,
    project: &ResolvedMiseProject,
    options: &SidecarOptions,
) -> Result<MaterializedWorkspace, SidecarError> {
    let discovered = discover_project_sidecars(project_root, project)?;
    materialize_discovered_sidecars(&discovered, options)
}

fn materialize_discovered_sidecars(
    discovered: &[DiscoveredSidecar],
    options: &SidecarOptions,
) -> Result<MaterializedWorkspace, SidecarError> {
    let temp_dir = tempfile::Builder::new()
        .prefix("trivy-plugin-mise-")
        .tempdir()
        .map_err(|source| SidecarError::Io {
            path: std::env::temp_dir().display().to_string(),
            source,
        })?;

    let mut entries = Vec::with_capacity(discovered.len());

    for sidecar in discovered {
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

/// Runs `trivy fs --format cyclonedx --quiet` on `path`, optionally skipping directories, and returns raw JSON.
pub fn run_trivy_fs_cyclonedx(
    path: &Path,
    skip_dirs: &[&str],
) -> Result<serde_json::Value, SidecarError> {
    run_trivy_fs_cyclonedx_with_args(path, skip_dirs, &[])
}

/// Runs `trivy fs --format cyclonedx --quiet` on `path` with additional raw Trivy arguments and returns raw JSON.
pub fn run_trivy_fs_cyclonedx_with_args(
    path: &Path,
    skip_dirs: &[&str],
    trivy_args: &[String],
) -> Result<serde_json::Value, SidecarError> {
    let mut cmd = Command::new("trivy");
    cmd.args(["fs", "--format", "cyclonedx", "--quiet"]);
    for dir in skip_dirs {
        cmd.args(["--skip-dirs", dir]);
    }
    cmd.args(trivy_args);
    cmd.arg(path);

    let output = cmd.output().map_err(SidecarError::TrivyInvocation)?;

    if !output.status.success() {
        return Err(SidecarError::TrivyFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let bom: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(bom)
}

/// Runs `trivy fs --format cyclonedx --quiet` on a materialized workspace and returns raw JSON.
pub fn extract_cyclonedx_json_from_workspace(
    workspace: &MaterializedWorkspace,
) -> Result<serde_json::Value, SidecarError> {
    extract_cyclonedx_json_from_workspace_with_args(workspace, &[])
}

/// Runs `trivy fs --format cyclonedx --quiet` on a materialized workspace with extra Trivy arguments and returns raw JSON.
pub fn extract_cyclonedx_json_from_workspace_with_args(
    workspace: &MaterializedWorkspace,
    trivy_args: &[String],
) -> Result<serde_json::Value, SidecarError> {
    let has_files = workspace
        .entries()
        .iter()
        .any(|entry| !entry.materialized_files.is_empty());
    if !has_files {
        return Ok(serde_json::json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.7",
            "components": [],
            "dependencies": []
        }));
    }

    run_trivy_fs_cyclonedx_with_args(workspace.path(), &[], trivy_args)
}

/// Runs `trivy fs --format cyclonedx --quiet` on a materialized workspace and parses the resulting BOM.
pub fn extract_cyclonedx_from_workspace(
    workspace: &MaterializedWorkspace,
) -> Result<TrivyCycloneDxBom, SidecarError> {
    let json = extract_cyclonedx_json_from_workspace(workspace)?;
    let bom: TrivyCycloneDxBom = serde_json::from_value(json)?;
    Ok(bom)
}

/// End-to-end helper: discovers sidecars, verifies digests, materializes into a temporary workspace,
/// extracts the raw CycloneDX JSON via Trivy, and cleans up the temporary workspace before returning.
pub fn extract_sidecar_json(
    project_root: &Path,
    lock: Option<&MiseLock>,
    options: &SidecarOptions,
) -> Result<serde_json::Value, SidecarError> {
    extract_sidecar_json_with_args(project_root, lock, options, &[])
}

/// End-to-end helper with additional raw Trivy arguments forwarded to `trivy fs --format cyclonedx`.
pub fn extract_sidecar_json_with_args(
    project_root: &Path,
    lock: Option<&MiseLock>,
    options: &SidecarOptions,
    trivy_args: &[String],
) -> Result<serde_json::Value, SidecarError> {
    let workspace = materialize_sidecars(project_root, lock, options)?;
    let bom = extract_cyclonedx_json_from_workspace_with_args(&workspace, trivy_args)?;
    workspace.close().map_err(|source| SidecarError::Io {
        path: std::env::temp_dir().display().to_string(),
        source,
    })?;
    Ok(bom)
}

/// End-to-end helper for a [`ResolvedMiseProject`] with additional raw Trivy arguments.
pub fn extract_project_sidecar_json_with_args(
    project_root: &Path,
    project: &ResolvedMiseProject,
    options: &SidecarOptions,
    trivy_args: &[String],
) -> Result<serde_json::Value, SidecarError> {
    let workspace = materialize_project_sidecars(project_root, project, options)?;
    let bom = extract_cyclonedx_json_from_workspace_with_args(&workspace, trivy_args)?;
    workspace.close().map_err(|source| SidecarError::Io {
        path: std::env::temp_dir().display().to_string(),
        source,
    })?;
    Ok(bom)
}

/// End-to-end helper: discovers sidecars, verifies digests, materializes into a temporary workspace,
/// extracts the CycloneDX BOM via Trivy, and cleans up the temporary workspace before returning.
pub fn extract_sidecar_bom(
    project_root: &Path,
    lock: Option<&MiseLock>,
    options: &SidecarOptions,
) -> Result<TrivyCycloneDxBom, SidecarError> {
    let json = extract_sidecar_json(project_root, lock, options)?;
    let bom: TrivyCycloneDxBom = serde_json::from_value(json)?;
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

fn ensure_within_root(
    allowed_root: &Path,
    canonical_dir: &Path,
    reported_path: &Path,
) -> Result<(), SidecarError> {
    let canonical_root = fs::canonicalize(allowed_root).map_err(|source| SidecarError::Io {
        path: allowed_root.display().to_string(),
        source,
    })?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(SidecarError::InvalidSidecarPath {
            path: reported_path.display().to_string(),
            reason: "sidecar path must not escape the project root".to_string(),
        });
    }
    Ok(())
}

fn materialized_subdir(rel_path: &Path, tool_name: &str, version: &str) -> PathBuf {
    for prefix in [
        Path::new(".config/mise/locks"),
        Path::new(".config/.mise/locks"),
        Path::new(".mise/locks"),
        Path::new("mise/locks"),
        Path::new("locks"),
    ] {
        if let Ok(stripped) = rel_path.strip_prefix(prefix)
            && !stripped.as_os_str().is_empty()
        {
            return stripped.to_path_buf();
        }
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
        let rel = Path::new(".mise/locks/npm-prettier/3.10.1");
        assert_eq!(
            materialized_subdir(rel, "npm:prettier", "3.10.1"),
            PathBuf::from("npm-prettier/3.10.1")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_sidecar_escaping_project_root() {
        let project = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let locks_tool_dir = project.path().join(".mise/locks/npm-prettier");
        fs::create_dir_all(&locks_tool_dir).unwrap();
        std::os::unix::fs::symlink(outside.path(), locks_tool_dir.join("3.10.1")).unwrap();

        let lock_toml = r#"
lockfile_version = 3

[[tools."npm:prettier"]]
version = "3.10.1"
aube = { path = ".mise/locks/npm-prettier/3.10.1" }
"#;
        let lock = MiseLock::from_str(lock_toml).unwrap();
        let err = discover_sidecars(project.path(), Some(&lock)).unwrap_err();
        assert!(matches!(err, SidecarError::InvalidSidecarPath { .. }));

        let scan_err = discover_sidecars(project.path(), None).unwrap_err();
        assert!(matches!(scan_err, SidecarError::InvalidSidecarPath { .. }));
    }
}
