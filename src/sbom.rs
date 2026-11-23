use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::component::{CycloneDxComponent, normalize_lock};
use crate::mise_lock::{MiseLock, MiseLockError};
use crate::purl::PurlError;
use crate::sidecar::{SidecarError, SidecarOptions, extract_sidecar_json, run_trivy_fs_cyclonedx};

const CYCLONEDX_1_7_SCHEMA: &str = "http://cyclonedx.org/schema/bom-1.7.schema.json";
const CYCLONEDX_SPEC_VERSION: &str = "1.7";
const MISE_TOOLCHAIN_REF: &str = "mise-toolchain";

/// Output mode for `trivy-plugin-mise sbom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum SbomMode {
    /// Combined product SBOM (`components[]`) and mise toolchain MBOM (`formulation[]`).
    #[default]
    Formulation,
    /// Standalone toolchain SBOM with mise tools and transitive packages in top-level `components[]`.
    Components,
}

/// Options controlling CycloneDX SBOM generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbomOptions {
    pub mode: SbomMode,
    pub target_platform: Option<String>,
    pub verify_digests: bool,
}

impl Default for SbomOptions {
    fn default() -> Self {
        Self {
            mode: SbomMode::Formulation,
            target_platform: None,
            verify_digests: true,
        }
    }
}

/// Errors returned while generating or writing a CycloneDX SBOM.
#[derive(Debug, Error)]
pub enum SbomError {
    #[error(transparent)]
    MiseLock(#[from] MiseLockError),

    #[error(transparent)]
    Purl(#[from] PurlError),

    #[error(transparent)]
    Sidecar(#[from] SidecarError),

    #[error("filesystem error at `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unexpected CycloneDX JSON structure: {0}")]
    InvalidBomStructure(String),
}

/// Generates a CycloneDX 1.7 SBOM for `project_root` according to `options`.
pub fn generate_sbom(project_root: &Path, options: &SbomOptions) -> Result<Value, SbomError> {
    let lock_path = project_root.join("mise.lock");
    let lock = if lock_path.is_file() {
        Some(MiseLock::from_path(&lock_path)?)
    } else {
        None
    };

    let mise_components = match &lock {
        Some(parsed) => normalize_lock(parsed, options.target_platform.as_deref())?,
        None => Vec::new(),
    };

    let sidecar_json = extract_sidecar_json(
        project_root,
        lock.as_ref(),
        &SidecarOptions {
            verify_digests: options.verify_digests,
        },
    )?;

    let include_dependencies = options.mode == SbomMode::Components;
    let merged = merge_toolchain_bom(&mise_components, &sidecar_json, include_dependencies)?;

    match options.mode {
        SbomMode::Formulation => build_formulation_bom(project_root, merged.components),
        SbomMode::Components => build_components_bom(project_root, merged),
    }
}

/// Writes a CycloneDX JSON document to `output` (or stdout when `None`).
pub fn write_sbom(bom: &Value, output: Option<&Path>) -> Result<(), SbomError> {
    let mut serialized = serde_json::to_string_pretty(bom)?;
    serialized.push('\n');

    match output {
        Some(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent).map_err(|source| SbomError::Io {
                    path: parent.display().to_string(),
                    source,
                })?;
            }
            fs::write(path, serialized).map_err(|source| SbomError::Io {
                path: path.display().to_string(),
                source,
            })?;
        }
        None => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(serialized.as_bytes())
                .map_err(|source| SbomError::Io {
                    path: "<stdout>".to_string(),
                    source,
                })?;
        }
    }

    Ok(())
}

struct MergedToolchain {
    components: Vec<Value>,
    dependencies: Vec<Value>,
}

fn merge_toolchain_bom(
    mise_components: &[CycloneDxComponent],
    sidecar_json: &Value,
    include_dependencies: bool,
) -> Result<MergedToolchain, SbomError> {
    let mut components = Vec::with_capacity(mise_components.len());
    let mut seen_refs = HashSet::new();
    let mut purl_to_index: HashMap<String, usize> = HashMap::new();
    let mut mise_root_refs = BTreeSet::new();

    for comp in mise_components {
        let val = serde_json::to_value(comp)?;
        let idx = components.len();
        seen_refs.insert(comp.bom_ref.clone());
        purl_to_index.insert(comp.purl.clone(), idx);
        mise_root_refs.insert(comp.bom_ref.clone());
        components.push(val);
    }

    let sidecar_components = sidecar_json
        .get("components")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    // Map original sidecar bom-ref -> retained bom-ref so dependency edges stay valid after deduplication.
    let mut sidecar_ref_remap: HashMap<String, String> = HashMap::new();

    for raw_comp in sidecar_components {
        let Some(purl) = raw_comp
            .get("purl")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
        else {
            continue;
        };

        let orig_ref = raw_comp
            .get("bom-ref")
            .and_then(Value::as_str)
            .unwrap_or(purl);

        if let Some(&existing_idx) = purl_to_index.get(purl) {
            if let Some(existing_ref) = components[existing_idx]
                .get("bom-ref")
                .and_then(Value::as_str)
                .map(str::to_string)
            {
                sidecar_ref_remap.insert(orig_ref.to_string(), existing_ref);
            }
            if let Some(licenses) = raw_comp.get("licenses")
                && components[existing_idx].get("licenses").is_none()
                && let Some(obj) = components[existing_idx].as_object_mut()
            {
                obj.insert("licenses".to_string(), licenses.clone());
            }
            continue;
        }

        if seen_refs.contains(orig_ref) {
            sidecar_ref_remap.insert(orig_ref.to_string(), orig_ref.to_string());
            continue;
        }

        let mut comp = raw_comp.clone();
        if comp.get("bom-ref").and_then(Value::as_str).is_none()
            && let Some(obj) = comp.as_object_mut()
        {
            obj.insert("bom-ref".to_string(), Value::String(orig_ref.to_string()));
        }

        let idx = components.len();
        seen_refs.insert(orig_ref.to_string());
        purl_to_index.insert(purl.to_string(), idx);
        sidecar_ref_remap.insert(orig_ref.to_string(), orig_ref.to_string());
        components.push(comp);
    }

    if !include_dependencies {
        return Ok(MergedToolchain {
            components,
            dependencies: Vec::new(),
        });
    }

    let mut dep_edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut has_incoming = HashSet::new();

    if let Some(raw_deps) = sidecar_json.get("dependencies").and_then(Value::as_array) {
        for dep in raw_deps {
            let Some(raw_ref) = dep.get("ref").and_then(Value::as_str) else {
                continue;
            };
            let Some(mapped_ref) = sidecar_ref_remap.get(raw_ref) else {
                continue;
            };
            let entry = dep_edges.entry(mapped_ref.clone()).or_default();

            if let Some(targets) = dep.get("dependsOn").and_then(Value::as_array) {
                for target in targets {
                    if let Some(raw_target) = target.as_str()
                        && let Some(mapped_target) = sidecar_ref_remap.get(raw_target)
                        && mapped_target != mapped_ref
                    {
                        entry.insert(mapped_target.clone());
                        has_incoming.insert(mapped_target.clone());
                    }
                }
            }
        }
    }

    let mut root_depends_on = mise_root_refs;
    for r in &seen_refs {
        if !has_incoming.contains(r) {
            root_depends_on.insert(r.clone());
        }
    }

    let mut dependencies = Vec::with_capacity(1 + dep_edges.len());
    dependencies.push(json!({
        "ref": MISE_TOOLCHAIN_REF,
        "dependsOn": root_depends_on.into_iter().collect::<Vec<_>>(),
    }));

    for (dep_ref, targets) in dep_edges {
        dependencies.push(json!({
            "ref": dep_ref,
            "dependsOn": targets.into_iter().collect::<Vec<_>>(),
        }));
    }

    Ok(MergedToolchain {
        components,
        dependencies,
    })
}

fn build_formulation_bom(
    project_root: &Path,
    toolchain_components: Vec<Value>,
) -> Result<Value, SbomError> {
    let mut base = run_trivy_fs_cyclonedx(project_root, &[".mise/locks"])?;
    let obj = base.as_object_mut().ok_or_else(|| {
        SbomError::InvalidBomStructure("Trivy CycloneDX output is not a JSON object".to_string())
    })?;

    obj.insert(
        "$schema".to_string(),
        Value::String(CYCLONEDX_1_7_SCHEMA.to_string()),
    );
    obj.insert(
        "specVersion".to_string(),
        Value::String(CYCLONEDX_SPEC_VERSION.to_string()),
    );

    if !toolchain_components.is_empty() {
        let formula = json!({
            "bom-ref": MISE_TOOLCHAIN_REF,
            "components": toolchain_components,
        });

        match obj.get_mut("formulation").and_then(Value::as_array_mut) {
            Some(existing) => existing.push(formula),
            None => {
                obj.insert("formulation".to_string(), Value::Array(vec![formula]));
            }
        }
    }

    Ok(base)
}

fn build_components_bom(project_root: &Path, merged: MergedToolchain) -> Result<Value, SbomError> {
    let project_name = project_root
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| MISE_TOOLCHAIN_REF.to_string());

    let serial_number = format!("urn:uuid:{}", generate_uuid_v4(&merged.components));

    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        Value::String(CYCLONEDX_1_7_SCHEMA.to_string()),
    );
    root.insert(
        "bomFormat".to_string(),
        Value::String("CycloneDX".to_string()),
    );
    root.insert(
        "specVersion".to_string(),
        Value::String(CYCLONEDX_SPEC_VERSION.to_string()),
    );
    root.insert("serialNumber".to_string(), Value::String(serial_number));
    root.insert("version".to_string(), json!(1));
    root.insert(
        "metadata".to_string(),
        json!({
            "timestamp": utc_now_rfc3339(),
            "tools": {
                "components": [
                    {
                        "type": "application",
                        "name": env!("CARGO_PKG_NAME"),
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                ]
            },
            "component": {
                "bom-ref": MISE_TOOLCHAIN_REF,
                "type": "application",
                "name": project_name,
            }
        }),
    );
    root.insert("components".to_string(), Value::Array(merged.components));
    root.insert(
        "dependencies".to_string(),
        Value::Array(merged.dependencies),
    );

    Ok(Value::Object(root))
}

fn generate_uuid_v4(components: &[Value]) -> String {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = Sha256::new();
    hasher.update(now_nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    if let Ok(serialized) = serde_json::to_vec(components) {
        hasher.update(serialized);
    }
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

fn utc_now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;

    // Howard Hinnant's civil-from-days algorithm (UTC).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{CycloneDxHash, CycloneDxProperty};

    #[test]
    fn merge_deduplicates_mise_tool_with_sidecar_package_and_remaps_deps() {
        let mise_components = vec![CycloneDxComponent {
            bom_ref: "pkg:npm/%40commitlint/cli@21.2.4".to_string(),
            component_type: "application".to_string(),
            name: "@commitlint/cli".to_string(),
            version: "21.2.4".to_string(),
            purl: "pkg:npm/%40commitlint/cli@21.2.4".to_string(),
            hashes: vec![CycloneDxHash {
                alg: "SHA-256".to_string(),
                content: "deadbeef".to_string(),
            }],
            external_references: Vec::new(),
            properties: vec![CycloneDxProperty {
                name: "mise:tool".to_string(),
                value: "npm:@commitlint/cli".to_string(),
            }],
        }];

        let sidecar_json = json!({
            "components": [
                {
                    "bom-ref": "lockfile-root-uuid",
                    "type": "application",
                    "name": "pnpm-lock.yaml"
                },
                {
                    "bom-ref": "pkg:npm/%40commitlint/cli@21.2.4",
                    "type": "library",
                    "name": "@commitlint/cli",
                    "version": "21.2.4",
                    "purl": "pkg:npm/%40commitlint/cli@21.2.4",
                    "licenses": [{ "license": { "id": "MIT" } }]
                },
                {
                    "bom-ref": "pkg:npm/chalk@5.3.0",
                    "type": "library",
                    "name": "chalk",
                    "version": "5.3.0",
                    "purl": "pkg:npm/chalk@5.3.0"
                }
            ],
            "dependencies": [
                {
                    "ref": "lockfile-root-uuid",
                    "dependsOn": ["pkg:npm/%40commitlint/cli@21.2.4"]
                },
                {
                    "ref": "pkg:npm/%40commitlint/cli@21.2.4",
                    "dependsOn": ["pkg:npm/chalk@5.3.0"]
                }
            ]
        });

        let merged = merge_toolchain_bom(&mise_components, &sidecar_json, true).unwrap();
        assert_eq!(merged.components.len(), 2);
        assert!(merged.components[0].get("licenses").is_some());
        assert_eq!(merged.components[1]["name"], "chalk");
        assert_eq!(merged.dependencies.len(), 2);

        let merged_no_deps = merge_toolchain_bom(&mise_components, &sidecar_json, false).unwrap();
        assert_eq!(merged_no_deps.components.len(), 2);
        assert!(merged_no_deps.dependencies.is_empty());
    }
}
