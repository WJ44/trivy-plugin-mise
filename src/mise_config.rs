use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::mise_lock::{LockedTool, MiseLock, MiseLockError};

/// Standard repository-scoped sidecar lock directories under a project root.
pub const STANDARD_SIDECAR_DIRS: &[&str] = &[
    ".config/mise/locks",
    ".config/.mise/locks",
    ".mise/locks",
    "mise/locks",
];

/// Standard non-local repository-scoped `mise` lockfiles in lowest-to-highest precedence order.
const STANDARD_LOCKFILES: &[&str] = &[
    ".config/mise/mise.lock",
    ".config/mise.lock",
    ".mise/mise.lock",
    "mise/mise.lock",
    "mise.lock",
];

/// Standard machine-local repository-scoped `mise` lockfiles in lowest-to-highest precedence order.
const STANDARD_LOCAL_LOCKFILES: &[&str] = &[
    ".config/mise/mise.local.lock",
    ".config/mise.local.lock",
    ".mise/mise.local.lock",
    "mise/mise.local.lock",
    "mise.local.lock",
];

/// Options controlling pure-filesystem `mise` config, lockfile, and sidecar discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryOptions {
    /// Include machine-local `*.local.toml` and `*.local.lock` files.
    pub include_local: bool,
    /// Explicit `mise` config file paths (overrides standard config discovery when non-empty).
    pub config_paths: Vec<PathBuf>,
    /// Explicit `mise` lockfile paths (overrides standard lockfile discovery when non-empty).
    pub lock_paths: Vec<PathBuf>,
}

/// Errors returned while discovering or parsing `mise` config files and lockfiles.
#[derive(Debug, Error)]
pub enum MiseConfigError {
    #[error("explicit mise config file `{path}` does not exist or is not a file")]
    MissingExplicitConfig { path: String },

    #[error("explicit mise lockfile `{path}` does not exist or is not a file")]
    MissingExplicitLock { path: String },

    #[error("failed to read mise config from `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse mise config `{path}`: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to parse mise lockfile `{path}`: {source}")]
    Lock {
        path: String,
        #[source]
        source: MiseLockError,
    },
}

/// A locked tool entry paired with the lockfile path it originated from.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLockedTool {
    pub tool: LockedTool,
    /// Path to the lockfile that supplied this tool entry.
    pub lock_path: PathBuf,
}

/// Record of a lockfile tool omitted because it is not declared in any active `mise` config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmittedLockTool {
    pub tool_name: String,
    pub lock_path: PathBuf,
}

/// Sidecar reference belonging to an omitted or superseded lockfile tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedSidecarRef {
    pub lock_path: PathBuf,
    pub sidecar_path: String,
}

/// Complete resolved `mise` project state from pure filesystem discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMiseProject {
    /// Discovered or explicitly configured `mise` config files (lowest-to-highest precedence).
    pub config_files: Vec<PathBuf>,
    /// Discovered or explicitly configured `mise` lockfiles (lowest-to-highest precedence).
    pub lock_files: Vec<PathBuf>,
    /// Active tool names declared across `config_files`, or `None` when zero config files exist.
    pub active_tools: Option<BTreeSet<String>>,
    /// Active locked tools merged across `lock_files` (higher-precedence lockfiles override lower-precedence ones),
    /// filtered against `active_tools` when `active_tools` is `Some(_)`.
    pub tools: Vec<ResolvedLockedTool>,
    /// Unconfigured/stale lockfile entries omitted due to `active_tools` filtering.
    pub omitted_tools: Vec<OmittedLockTool>,
    /// Sidecar references from omitted or superseded lockfile entries that must be excluded from discovery.
    pub excluded_sidecars: Vec<ExcludedSidecarRef>,
    /// Existing sidecar lock directories to consider for fallback directory scanning.
    pub sidecar_lock_dirs: Vec<PathBuf>,
    /// Relative directory paths to pass to `trivy fs --skip-dirs` during Tier 1 repo scans.
    pub skip_dirs: Vec<String>,
}

/// Discovers standard repository-scoped `mise` configuration files in lowest-to-highest precedence order.
///
/// Follows <https://mise.jdx.dev/configuration.html> for repository-scoped paths.
/// Machine-local `*.local.toml` files are excluded unless `include_local` is `true`.
pub fn discover_standard_config_files(
    project_root: &Path,
    include_local: bool,
) -> Result<Vec<PathBuf>, MiseConfigError> {
    let mut files = Vec::new();

    // Non-local files in mise's standard lowest-to-highest precedence order.
    collect_conf_d_files(&project_root.join(".config/mise/conf.d"), false, &mut files)?;
    push_if_file(project_root.join(".config/mise/config.toml"), &mut files);
    push_if_file(project_root.join(".config/mise/mise.toml"), &mut files);
    push_if_file(project_root.join(".config/mise.toml"), &mut files);
    collect_conf_d_files(&project_root.join(".mise/conf.d"), false, &mut files)?;
    push_if_file(project_root.join(".mise/config.toml"), &mut files);
    collect_conf_d_files(&project_root.join("mise/conf.d"), false, &mut files)?;
    push_if_file(project_root.join("mise/config.toml"), &mut files);
    push_if_file(project_root.join("mise.toml"), &mut files);
    push_if_file(project_root.join(".mise.toml"), &mut files);

    if include_local {
        collect_conf_d_files(&project_root.join(".config/mise/conf.d"), true, &mut files)?;
        push_if_file(
            project_root.join(".config/mise/config.local.toml"),
            &mut files,
        );
        push_if_file(
            project_root.join(".config/mise/mise.local.toml"),
            &mut files,
        );
        push_if_file(project_root.join(".config/mise.local.toml"), &mut files);
        collect_conf_d_files(&project_root.join(".mise/conf.d"), true, &mut files)?;
        push_if_file(project_root.join(".mise/config.local.toml"), &mut files);
        collect_conf_d_files(&project_root.join("mise/conf.d"), true, &mut files)?;
        push_if_file(project_root.join("mise/config.local.toml"), &mut files);
        push_if_file(project_root.join("mise.local.toml"), &mut files);
        push_if_file(project_root.join(".mise.local.toml"), &mut files);
    }

    Ok(files)
}

/// Discovers standard repository-scoped `mise` lockfiles in lowest-to-highest precedence order.
///
/// Machine-local `*.local.lock` files are excluded unless `include_local` is `true`.
#[must_use]
pub fn discover_standard_lock_files(project_root: &Path, include_local: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for rel in STANDARD_LOCKFILES {
        push_if_file(project_root.join(rel), &mut files);
    }
    if include_local {
        for rel in STANDARD_LOCAL_LOCKFILES {
            push_if_file(project_root.join(rel), &mut files);
        }
    }
    files
}

/// Discovers standard sidecar lock directories that exist under `project_root`.
#[must_use]
pub fn discover_standard_sidecar_dirs(project_root: &Path) -> Vec<PathBuf> {
    STANDARD_SIDECAR_DIRS
        .iter()
        .map(|rel| project_root.join(rel))
        .filter(|path| path.is_dir())
        .collect()
}

/// Returns relative sidecar directory paths under `project_root` that Tier 1 `trivy fs`
/// repository scans should skip (always including `.mise/locks` as a baseline).
#[must_use]
pub fn discover_standard_skip_dirs(project_root: &Path) -> Vec<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(".mise/locks".to_string());
    for rel in STANDARD_SIDECAR_DIRS {
        if project_root.join(rel).is_dir() {
            dirs.insert((*rel).to_string());
        }
    }
    dirs.into_iter().collect()
}

/// Parses a `mise` TOML configuration string and returns the set of declared tool names
/// from `[tools]` and `[tasks.<name>.tools]`.
pub fn parse_mise_config_tools_str(content: &str) -> Result<BTreeSet<String>, toml::de::Error> {
    let value: toml::Value = toml::from_str(content)?;
    let mut tools = BTreeSet::new();

    if let Some(tools_table) = value.get("tools").and_then(toml::Value::as_table) {
        for (tool_name, spec) in tools_table {
            if !matches!(spec, toml::Value::Array(arr) if arr.is_empty()) {
                tools.insert(tool_name.clone());
            }
        }
    }

    if let Some(tasks_table) = value.get("tasks").and_then(toml::Value::as_table) {
        for task_val in tasks_table.values() {
            if let Some(task_tools) = task_val
                .as_table()
                .and_then(|t| t.get("tools"))
                .and_then(toml::Value::as_table)
            {
                for (tool_name, spec) in task_tools {
                    if !matches!(spec, toml::Value::Array(arr) if arr.is_empty()) {
                        tools.insert(tool_name.clone());
                    }
                }
            }
        }
    }

    Ok(tools)
}

/// Reads and parses a `mise` TOML configuration file from disk, returning its declared tool names.
pub fn parse_mise_config_tools(path: &Path) -> Result<BTreeSet<String>, MiseConfigError> {
    let content = fs::read_to_string(path).map_err(|source| MiseConfigError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_mise_config_tools_str(&content).map_err(|source| MiseConfigError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Discovers, parses, merges, and filters `mise` configs, lockfiles, and sidecar skip directories
/// for `project_root` via pure filesystem parsing.
pub fn resolve_mise_project(
    project_root: &Path,
    options: &DiscoveryOptions,
) -> Result<ResolvedMiseProject, MiseConfigError> {
    let config_files = if !options.config_paths.is_empty() {
        resolve_explicit_paths(project_root, &options.config_paths, true)?
    } else {
        discover_standard_config_files(project_root, options.include_local)?
    };

    let active_tools = if config_files.is_empty() {
        None
    } else {
        let mut set = BTreeSet::new();
        for cfg_path in &config_files {
            set.extend(parse_mise_config_tools(cfg_path)?);
        }
        Some(set)
    };

    let explicit_locks = !options.lock_paths.is_empty();
    let lock_files = if explicit_locks {
        resolve_explicit_paths(project_root, &options.lock_paths, false)?
    } else {
        discover_standard_lock_files(project_root, options.include_local)
    };

    let mut merged_by_name: BTreeMap<String, Vec<ResolvedLockedTool>> = BTreeMap::new();
    let mut omitted_tools = Vec::new();
    let mut excluded_sidecars = Vec::new();
    let mut all_sidecar_refs: Vec<(PathBuf, String)> = Vec::new();

    for lock_path in &lock_files {
        let parsed = MiseLock::from_path(lock_path).map_err(|source| MiseConfigError::Lock {
            path: lock_path.display().to_string(),
            source,
        })?;

        for (tool_name, entries) in parsed.tools {
            for entry in &entries {
                for sidecar in entry.sidecars.values() {
                    all_sidecar_refs.push((lock_path.clone(), sidecar.path.clone()));
                }
            }

            let is_active = match &active_tools {
                None => true,
                Some(active) => {
                    active.contains(&tool_name)
                        || entries.iter().any(|entry| {
                            entry
                                .backend
                                .as_deref()
                                .is_some_and(|backend| active.contains(backend))
                        })
                }
            };

            if !is_active {
                omitted_tools.push(OmittedLockTool {
                    tool_name,
                    lock_path: lock_path.clone(),
                });
                for entry in entries {
                    for sidecar in entry.sidecars.into_values() {
                        excluded_sidecars.push(ExcludedSidecarRef {
                            lock_path: lock_path.clone(),
                            sidecar_path: sidecar.path,
                        });
                    }
                }
                continue;
            }

            let resolved_entries: Vec<ResolvedLockedTool> = entries
                .into_iter()
                .map(|tool| ResolvedLockedTool {
                    tool,
                    lock_path: lock_path.clone(),
                })
                .collect();

            if let Some(superseded) = merged_by_name.insert(tool_name, resolved_entries) {
                for old in superseded {
                    for sidecar in old.tool.sidecars.into_values() {
                        excluded_sidecars.push(ExcludedSidecarRef {
                            lock_path: old.lock_path.clone(),
                            sidecar_path: sidecar.path,
                        });
                    }
                }
            }
        }
    }

    let tools: Vec<ResolvedLockedTool> = merged_by_name.into_values().flatten().collect();

    let sidecar_lock_dirs = if explicit_locks {
        Vec::new()
    } else {
        discover_standard_sidecar_dirs(project_root)
    };

    // When local files are excluded during standard discovery, record sidecar paths from any
    // ignored `*.local.lock` files so fallback directory scanning does not accidentally pick them
    // up in lockfile-only repositories without config files.
    if !options.include_local && !explicit_locks {
        for rel in STANDARD_LOCAL_LOCKFILES {
            let local_lock = project_root.join(rel);
            if local_lock.is_file()
                && let Ok(parsed) = MiseLock::from_path(&local_lock)
            {
                for tool in parsed.iter_tools() {
                    for sidecar in tool.sidecars.values() {
                        excluded_sidecars.push(ExcludedSidecarRef {
                            lock_path: local_lock.clone(),
                            sidecar_path: sidecar.path.clone(),
                        });
                    }
                }
            }
        }
    }

    let skip_dirs = compute_skip_dirs(project_root, &all_sidecar_refs);

    Ok(ResolvedMiseProject {
        config_files,
        lock_files,
        active_tools,
        tools,
        omitted_tools,
        excluded_sidecars,
        sidecar_lock_dirs,
        skip_dirs,
    })
}

/// Writes warnings for omitted unconfigured lockfile tools to `writer`.
pub fn write_omitted_tool_warnings<W: Write>(
    writer: &mut W,
    project_root: &Path,
    omitted: &[OmittedLockTool],
) -> io::Result<()> {
    for item in omitted {
        let display_lock = item
            .lock_path
            .strip_prefix(project_root)
            .unwrap_or(&item.lock_path)
            .display();
        writeln!(
            writer,
            "warning: omitting unconfigured tool `{}` from `{display_lock}` (not declared in active mise config)",
            item.tool_name
        )?;
    }
    writer.flush()
}

/// Emits warnings for omitted unconfigured lockfile tools to `stderr`.
pub fn emit_omitted_tool_warnings(project_root: &Path, omitted: &[OmittedLockTool]) {
    if omitted.is_empty() {
        return;
    }
    let mut stderr = io::stderr().lock();
    let _ = write_omitted_tool_warnings(&mut stderr, project_root, omitted);
}

/// Checks whether a sidecar directory segment (e.g. `npm-prettier`, `npm-commitlint-cli`)
/// corresponds to any tool declared in `active_tools`.
#[must_use]
pub fn sidecar_dir_matches_active_tools(dir_name: &str, active_tools: &BTreeSet<String>) -> bool {
    active_tools.iter().any(|tool| {
        tool == dir_name
            || sanitize_tool_dir_name(tool) == dir_name
            || tool.replace(['/', ':', '@'], "-") == dir_name
    })
}

/// Normalizes a `mise` tool name into the directory segment format used under `locks/`
/// (for example `npm:@commitlint/cli` -> `npm-commitlint-cli`, `npm:prettier` -> `npm-prettier`).
#[must_use]
pub fn sanitize_tool_dir_name(tool_name: &str) -> String {
    let mut out = String::with_capacity(tool_name.len());
    let mut prev_dash = false;
    for ch in tool_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn resolve_explicit_paths(
    project_root: &Path,
    paths: &[PathBuf],
    is_config: bool,
) -> Result<Vec<PathBuf>, MiseConfigError> {
    let mut resolved = Vec::with_capacity(paths.len());
    for raw in paths {
        let candidate = if raw.is_absolute() {
            raw.clone()
        } else {
            let under_root = project_root.join(raw);
            if under_root.is_file() {
                under_root
            } else if raw.is_file() {
                raw.clone()
            } else {
                under_root
            }
        };

        if !candidate.is_file() {
            return Err(if is_config {
                MiseConfigError::MissingExplicitConfig {
                    path: raw.display().to_string(),
                }
            } else {
                MiseConfigError::MissingExplicitLock {
                    path: raw.display().to_string(),
                }
            });
        }
        resolved.push(candidate);
    }
    Ok(resolved)
}

fn collect_conf_d_files(
    dir: &Path,
    local_only: bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), MiseConfigError> {
    if !dir.is_dir() {
        return Ok(());
    }

    let entries = fs::read_dir(dir).map_err(|source| MiseConfigError::Io {
        path: dir.display().to_string(),
        source,
    })?;

    let mut matched = Vec::new();
    for entry_res in entries {
        let entry = entry_res.map_err(|source| MiseConfigError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".toml") {
            continue;
        }
        let is_local = name.ends_with(".local.toml");
        if is_local == local_only {
            matched.push(path);
        }
    }

    matched.sort();
    out.extend(matched);
    Ok(())
}

fn push_if_file(path: PathBuf, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path);
    }
}

fn compute_skip_dirs(project_root: &Path, sidecar_refs: &[(PathBuf, String)]) -> Vec<String> {
    let mut skip_set: BTreeSet<String> = discover_standard_skip_dirs(project_root)
        .into_iter()
        .collect();

    let canonical_root = fs::canonicalize(project_root).ok();

    for (lock_path, raw_sidecar_path) in sidecar_refs {
        let rel = Path::new(raw_sidecar_path);
        if raw_sidecar_path.trim().is_empty()
            || rel.is_absolute()
            || rel.components().any(|c| matches!(c, Component::ParentDir))
        {
            continue;
        }

        let lock_dir = lock_path.parent().unwrap_or(project_root);
        let candidate = {
            let under_lock = lock_dir.join(rel);
            if under_lock.is_dir() {
                under_lock
            } else {
                project_root.join(rel)
            }
        };

        if !candidate.is_dir() {
            continue;
        }

        // Convert to a clean path relative to project_root when inside the project.
        let rel_to_root = if let Ok(stripped) = candidate.strip_prefix(project_root) {
            Some(stripped.to_path_buf())
        } else if let (Some(root_canon), Ok(cand_canon)) =
            (&canonical_root, fs::canonicalize(&candidate))
        {
            cand_canon
                .strip_prefix(root_canon)
                .ok()
                .map(Path::to_path_buf)
        } else {
            None
        };

        let Some(rel_dir) = rel_to_root else {
            continue;
        };
        let normalized = normalize_rel_path(&rel_dir);
        if normalized.is_empty() {
            continue;
        }

        // Skip adding a nested sidecar dir if an ancestor skip dir already covers it.
        let already_covered = skip_set.iter().any(|existing| {
            normalized == *existing || normalized.starts_with(&format!("{existing}/"))
        });
        if !already_covered {
            // If the sidecar path starts with `locks/` under a subdirectory, skip that `locks` root.
            if rel.starts_with("locks")
                && let Some(lock_dir_rel) = lock_dir
                    .strip_prefix(project_root)
                    .ok()
                    .map(Path::to_path_buf)
                    .or_else(|| {
                        canonical_root.as_ref().and_then(|root_canon| {
                            fs::canonicalize(lock_dir).ok().and_then(|d| {
                                d.strip_prefix(root_canon).ok().map(Path::to_path_buf)
                            })
                        })
                    })
            {
                let locks_root = normalize_rel_path(&lock_dir_rel.join("locks"));
                if !locks_root.is_empty() && project_root.join(&locks_root).is_dir() {
                    skip_set.insert(locks_root);
                    continue;
                }
            }
            skip_set.insert(normalized);
        }
    }

    skip_set.into_iter().collect()
}

fn normalize_rel_path(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(seg) => Some(seg.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn discovers_standard_layered_configs_in_precedence_order_and_excludes_local_by_default() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let paths = [
            ".config/mise/conf.d/20-extra.toml",
            ".config/mise/conf.d/10-base.toml",
            ".config/mise/conf.d/30-local.local.toml",
            ".config/mise/conf.d/README.md",
            ".config/mise/config.toml",
            ".config/mise/config.local.toml",
            ".config/mise/mise.toml",
            ".config/mise/mise.local.toml",
            ".config/mise.toml",
            ".config/mise.local.toml",
            ".mise/conf.d/01-shared.toml",
            ".mise/config.toml",
            ".mise/config.local.toml",
            "mise/conf.d/01-project.toml",
            "mise/config.toml",
            "mise/config.local.toml",
            "mise.toml",
            "mise.local.toml",
            ".mise.toml",
            ".mise.local.toml",
        ];

        for rel in paths {
            let full = root.join(rel);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, "").unwrap();
        }

        let default_configs: Vec<String> = discover_standard_config_files(root, false)
            .unwrap()
            .into_iter()
            .map(|p| normalize_rel_path(p.strip_prefix(root).unwrap()))
            .collect();

        assert_eq!(
            default_configs,
            vec![
                ".config/mise/conf.d/10-base.toml",
                ".config/mise/conf.d/20-extra.toml",
                ".config/mise/config.toml",
                ".config/mise/mise.toml",
                ".config/mise.toml",
                ".mise/conf.d/01-shared.toml",
                ".mise/config.toml",
                "mise/conf.d/01-project.toml",
                "mise/config.toml",
                "mise.toml",
                ".mise.toml",
            ]
        );

        let with_local: Vec<String> = discover_standard_config_files(root, true)
            .unwrap()
            .into_iter()
            .map(|p| normalize_rel_path(p.strip_prefix(root).unwrap()))
            .collect();

        assert_eq!(
            with_local,
            vec![
                ".config/mise/conf.d/10-base.toml",
                ".config/mise/conf.d/20-extra.toml",
                ".config/mise/config.toml",
                ".config/mise/mise.toml",
                ".config/mise.toml",
                ".mise/conf.d/01-shared.toml",
                ".mise/config.toml",
                "mise/conf.d/01-project.toml",
                "mise/config.toml",
                "mise.toml",
                ".mise.toml",
                ".config/mise/conf.d/30-local.local.toml",
                ".config/mise/config.local.toml",
                ".config/mise/mise.local.toml",
                ".config/mise.local.toml",
                ".mise/config.local.toml",
                "mise/config.local.toml",
                "mise.local.toml",
                ".mise.local.toml",
            ]
        );
    }

    #[test]
    fn discovers_standard_lockfiles_with_local_opt_in() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        for rel in [
            ".config/mise/mise.lock",
            ".config/mise/mise.local.lock",
            ".config/mise.lock",
            ".config/mise.local.lock",
            ".mise/mise.lock",
            ".mise/mise.local.lock",
            "mise/mise.lock",
            "mise/mise.local.lock",
            "mise.lock",
            "mise.local.lock",
        ] {
            let full = root.join(rel);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, "lockfile_version = 3\n").unwrap();
        }

        let default_locks: Vec<String> = discover_standard_lock_files(root, false)
            .into_iter()
            .map(|p| normalize_rel_path(p.strip_prefix(root).unwrap()))
            .collect();
        assert_eq!(
            default_locks,
            vec![
                ".config/mise/mise.lock",
                ".config/mise.lock",
                ".mise/mise.lock",
                "mise/mise.lock",
                "mise.lock",
            ]
        );

        let with_local: Vec<String> = discover_standard_lock_files(root, true)
            .into_iter()
            .map(|p| normalize_rel_path(p.strip_prefix(root).unwrap()))
            .collect();
        assert_eq!(
            with_local,
            vec![
                ".config/mise/mise.lock",
                ".config/mise.lock",
                ".mise/mise.lock",
                "mise/mise.lock",
                "mise.lock",
                ".config/mise/mise.local.lock",
                ".config/mise.local.lock",
                ".mise/mise.local.lock",
                "mise/mise.local.lock",
                "mise.local.lock",
            ]
        );
    }

    #[test]
    fn parses_tools_from_tools_table_and_task_tools() {
        let content = r#"
[tools]
node = "26.13.1"
"npm:prettier" = "3.10.1"
rust = { version = "1.100.0", components = ["rustfmt", "clippy"] }
python = ["3.11", "3.12"]
disabled = []

[tasks.lint]
tools = { shellcheck = "0.11.0" }
run = "shellcheck"

[tasks.fmt.tools]
shfmt = "3.14.1"
"#;

        let tools = parse_mise_config_tools_str(content).unwrap();
        assert_eq!(
            tools,
            BTreeSet::from([
                "node".to_string(),
                "npm:prettier".to_string(),
                "python".to_string(),
                "rust".to_string(),
                "shellcheck".to_string(),
                "shfmt".to_string(),
            ])
        );
    }

    #[test]
    fn filters_unconfigured_lockfile_tools_and_preserves_lockfile_only_projects() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let lock_toml = r#"
lockfile_version = 3

[[tools.rust]]
version = "1.100.0"
backend = "core:rust"

[[tools.ruff]]
version = "0.17.4"
backend = "aqua:astral-sh/ruff"
"#;
        fs::write(root.join("mise.lock"), lock_toml).unwrap();

        // Lockfile-only: no config files -> keep all locked tools, no warnings.
        let lock_only = resolve_mise_project(root, &DiscoveryOptions::default()).unwrap();
        assert!(lock_only.active_tools.is_none());
        assert_eq!(lock_only.tools.len(), 2);
        assert!(lock_only.omitted_tools.is_empty());

        // Add mise.toml declaring only `rust` -> `ruff` is omitted and warned.
        fs::write(root.join("mise.toml"), "[tools]\nrust = \"1.100.0\"\n").unwrap();
        let filtered = resolve_mise_project(root, &DiscoveryOptions::default()).unwrap();
        assert_eq!(
            filtered.active_tools,
            Some(BTreeSet::from(["rust".to_string()]))
        );
        assert_eq!(filtered.tools.len(), 1);
        assert_eq!(filtered.tools[0].tool.name, "rust");
        assert_eq!(filtered.omitted_tools.len(), 1);
        assert_eq!(filtered.omitted_tools[0].tool_name, "ruff");

        let mut buf = Vec::new();
        write_omitted_tool_warnings(&mut buf, root, &filtered.omitted_tools).unwrap();
        let warning = String::from_utf8(buf).unwrap();
        assert!(warning.contains("omitting unconfigured tool `ruff` from `mise.lock`"));
    }

    #[test]
    fn higher_precedence_lockfile_overrides_lower_precedence_entry() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".config/mise/conf.d")).unwrap();
        fs::write(
            root.join(".config/mise/conf.d/base.toml"),
            "[tools]\nnode = \"24.0.0\"\nlefthook = \"2.2.2\"\n",
        )
        .unwrap();
        fs::write(
            root.join(".config/mise/mise.lock"),
            r#"
lockfile_version = 3

[[tools.node]]
version = "24.0.0"
backend = "core:node"

[[tools.lefthook]]
version = "2.2.2"
backend = "aqua:evilmartians/lefthook"
"#,
        )
        .unwrap();

        fs::write(
            root.join("mise.toml"),
            "[tools]\nnode = \"26.13.1\"\nrust = \"1.100.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("mise.lock"),
            r#"
lockfile_version = 3

[[tools.node]]
version = "26.13.1"
backend = "core:node"

[[tools.rust]]
version = "1.100.0"
backend = "core:rust"
"#,
        )
        .unwrap();

        let project = resolve_mise_project(root, &DiscoveryOptions::default()).unwrap();
        assert!(project.omitted_tools.is_empty());
        let versions: BTreeMap<&str, &str> = project
            .tools
            .iter()
            .map(|t| (t.tool.name.as_str(), t.tool.version.as_str()))
            .collect();
        assert_eq!(
            versions,
            BTreeMap::from([
                ("lefthook", "2.2.2"),
                ("node", "26.13.1"),
                ("rust", "1.100.0"),
            ])
        );
    }

    #[test]
    fn sanitize_tool_dir_name_matches_mise_conventions() {
        assert_eq!(
            sanitize_tool_dir_name("npm:@commitlint/cli"),
            "npm-commitlint-cli"
        );
        assert_eq!(sanitize_tool_dir_name("npm:prettier"), "npm-prettier");
        assert_eq!(sanitize_tool_dir_name("pipx:black"), "pipx-black");
    }
}
