use std::collections::BTreeSet;

use serde::Serialize;

use crate::mise_lock::{LockedTool, MiseLock, PlatformAsset};
use crate::purl::{PurlError, tool_to_purl};

/// Structured CycloneDX component representation for a locked `mise` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDxComponent {
    #[serde(rename = "bom-ref")]
    pub bom_ref: String,
    #[serde(rename = "type")]
    pub component_type: String,
    pub name: String,
    pub version: String,
    pub purl: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hashes: Vec<CycloneDxHash>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub external_references: Vec<CycloneDxExternalReference>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<CycloneDxProperty>,
}

/// CycloneDX cryptographic hash entry (`alg` + hex `content`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CycloneDxHash {
    pub alg: String,
    pub content: String,
}

/// CycloneDX external reference entry (for example distribution download URLs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CycloneDxExternalReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// CycloneDX key-value property entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CycloneDxProperty {
    pub name: String,
    pub value: String,
}

/// Normalizes all tool entries in a [`MiseLock`] into [`CycloneDxComponent`]s.
///
/// When `target_platform` is `Some(triple)`, platform-specific `hashes`,
/// `download_url` PURL qualifiers, and top-level `mise:provenance` are derived
/// from that platform. When `None`, all platform metadata is still recorded in
/// `properties` and `external_references`, and `mise:provenance` is set if all
/// provenance-bearing platforms agree on a single provenance type.
pub fn normalize_lock(
    lock: &MiseLock,
    target_platform: Option<&str>,
) -> Result<Vec<CycloneDxComponent>, PurlError> {
    normalize_tools(lock.iter_tools(), target_platform)
}

/// Normalizes an iterator of [`LockedTool`] entries into [`CycloneDxComponent`]s.
pub fn normalize_tools<'a>(
    tools: impl IntoIterator<Item = &'a LockedTool>,
    target_platform: Option<&str>,
) -> Result<Vec<CycloneDxComponent>, PurlError> {
    tools
        .into_iter()
        .map(|tool| normalize_tool(tool, target_platform))
        .collect()
}

/// Normalizes a single [`LockedTool`] into a [`CycloneDxComponent`].
pub fn normalize_tool(
    tool: &LockedTool,
    target_platform: Option<&str>,
) -> Result<CycloneDxComponent, PurlError> {
    let purl = tool_to_purl(tool, target_platform)?.to_string();
    let backend = tool.effective_backend();

    let display_name = if let Some(npm_pkg) = backend.strip_prefix("npm:") {
        npm_pkg.to_string()
    } else {
        tool.name.clone()
    };

    let mut properties = vec![
        CycloneDxProperty {
            name: "mise:tool".to_string(),
            value: tool.name.clone(),
        },
        CycloneDxProperty {
            name: "mise:backend".to_string(),
            value: backend.to_string(),
        },
    ];

    if let Some(provenance) = resolve_provenance(tool, target_platform) {
        properties.push(CycloneDxProperty {
            name: "mise:provenance".to_string(),
            value: provenance,
        });
    }

    if let Some(pkg_provenance) = &tool.provenance {
        properties.push(CycloneDxProperty {
            name: "mise:provenance:repository".to_string(),
            value: pkg_provenance.repository.clone(),
        });
    }

    for (sidecar_name, sidecar) in &tool.sidecars {
        properties.push(CycloneDxProperty {
            name: format!("mise:{sidecar_name}:path"),
            value: sidecar.path.clone(),
        });
        if let Some(digest) = &sidecar.digest {
            properties.push(CycloneDxProperty {
                name: format!("mise:{sidecar_name}:digest"),
                value: digest.clone(),
            });
        }
    }

    let mut hashes = Vec::new();
    let mut external_references = Vec::new();

    if let Some(plat) = target_platform {
        if let Some(asset) = tool.platforms.get(plat) {
            if let Some(checksum) = &asset.checksum
                && let Some(hash) = parse_checksum_to_hash(checksum)
            {
                hashes.push(hash);
            }
            push_platform_metadata(plat, asset, &mut properties, &mut external_references);
        }
    } else {
        for (plat, asset) in &tool.platforms {
            push_platform_metadata(plat, asset, &mut properties, &mut external_references);
        }
    }

    Ok(CycloneDxComponent {
        bom_ref: purl.clone(),
        component_type: "application".to_string(),
        name: display_name,
        version: tool.version.clone(),
        purl,
        hashes,
        external_references,
        properties,
    })
}

/// Parses a `mise.lock` checksum string (e.g. `"sha256:abcdef..."`) into a [`CycloneDxHash`].
pub fn parse_checksum_to_hash(checksum: &str) -> Option<CycloneDxHash> {
    let (alg_raw, digest) = checksum.split_once(':')?;
    let digest = digest.trim();
    if digest.is_empty() {
        return None;
    }

    let alg = match alg_raw.trim().to_ascii_lowercase().as_str() {
        "sha256" => "SHA-256",
        "sha384" => "SHA-384",
        "sha512" => "SHA-512",
        "sha1" => "SHA-1",
        "md5" => "MD5",
        _ => return None,
    };

    Some(CycloneDxHash {
        alg: alg.to_string(),
        content: digest.to_string(),
    })
}

fn resolve_provenance(tool: &LockedTool, target_platform: Option<&str>) -> Option<String> {
    if let Some(plat) = target_platform {
        return tool
            .platforms
            .get(plat)
            .and_then(|asset| asset.provenance.clone());
    }

    let unique: BTreeSet<&str> = tool
        .platforms
        .values()
        .filter_map(|asset| asset.provenance.as_deref())
        .collect();

    if unique.len() == 1 {
        unique.into_iter().next().map(str::to_string)
    } else {
        None
    }
}

fn push_platform_metadata(
    platform: &str,
    asset: &PlatformAsset,
    properties: &mut Vec<CycloneDxProperty>,
    external_references: &mut Vec<CycloneDxExternalReference>,
) {
    if let Some(checksum) = &asset.checksum {
        properties.push(CycloneDxProperty {
            name: format!("mise:platform:{platform}:checksum"),
            value: checksum.clone(),
        });
    }
    if let Some(url) = &asset.url {
        properties.push(CycloneDxProperty {
            name: format!("mise:platform:{platform}:url"),
            value: url.clone(),
        });
        external_references.push(CycloneDxExternalReference {
            ref_type: "distribution".to_string(),
            url: url.clone(),
            comment: Some(format!("Platform {platform} download")),
        });
    }
    if let Some(provenance) = &asset.provenance {
        properties.push(CycloneDxProperty {
            name: format!("mise:platform:{platform}:provenance"),
            value: provenance.clone(),
        });
    }
    if let Some(source) = &asset.checksum_source {
        properties.push(CycloneDxProperty {
            name: format!("mise:platform:{platform}:checksum_source"),
            value: source.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256_checksum_to_cyclonedx_hash() {
        let hash = parse_checksum_to_hash(
            "sha256:2b8d7fd52771f26d7a50260ef10cd6fe704d66f60102e566362b84da09884ab7",
        )
        .unwrap();
        assert_eq!(hash.alg, "SHA-256");
        assert_eq!(
            hash.content,
            "2b8d7fd52771f26d7a50260ef10cd6fe704d66f60102e566362b84da09884ab7"
        );
    }
}
