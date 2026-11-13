use std::fmt;

use thiserror::Error;

use crate::mise_lock::LockedTool;

/// Errors produced when mapping a `mise.lock` tool entry to a Package URL (PURL).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PurlError {
    #[error("cannot build PURL for tool `{tool}`: version is empty")]
    EmptyVersion { tool: String },

    #[error("cannot build PURL for tool `{tool}` with backend `{backend}`: {reason}")]
    InvalidBackend {
        tool: String,
        backend: String,
        reason: String,
    },
}

/// Canonical Package URL (`pkg:<type>/...`) representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageUrl {
    pub pkg_type: String,
    pub namespace: Option<String>,
    pub name: String,
    pub version: String,
    pub qualifiers: Vec<(String, String)>,
    pub subpath: Option<String>,
}

impl fmt::Display for PackageUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pkg:{}/", self.pkg_type)?;
        if let Some(ns) = &self.namespace {
            write!(f, "{ns}/")?;
        }
        write!(f, "{}@{}", self.name, percent_encode(&self.version))?;
        if !self.qualifiers.is_empty() {
            f.write_str("?")?;
            for (i, (k, v)) in self.qualifiers.iter().enumerate() {
                if i > 0 {
                    f.write_str("&")?;
                }
                write!(f, "{k}={}", percent_encode(v))?;
            }
        }
        if let Some(subpath) = &self.subpath {
            write!(f, "#{subpath}")?;
        }
        Ok(())
    }
}

/// Maps a [`LockedTool`] to a canonical [`PackageUrl`].
///
/// When `platform` is `Some(triple)`, any `url` on that platform entry is used
/// as `download_url` for `generic` PURLs (such as `core:` backends).
pub fn tool_to_purl(tool: &LockedTool, platform: Option<&str>) -> Result<PackageUrl, PurlError> {
    let download_url = platform
        .and_then(|plat| tool.platforms.get(plat))
        .and_then(|asset| asset.url.as_deref());

    backend_to_purl(
        tool.effective_backend(),
        &tool.name,
        &tool.version,
        download_url,
    )
}

/// Maps a `mise` backend string, tool name, version, and optional download URL to a [`PackageUrl`].
pub fn backend_to_purl(
    backend: &str,
    tool_name: &str,
    version: &str,
    download_url: Option<&str>,
) -> Result<PackageUrl, PurlError> {
    let version = version.trim();
    if version.is_empty() {
        return Err(PurlError::EmptyVersion {
            tool: tool_name.to_string(),
        });
    }

    if let Some(pkg) = backend.strip_prefix("npm:") {
        return map_npm(pkg, tool_name, backend, version);
    }

    if let Some(pkg) = backend
        .strip_prefix("pipx:")
        .or_else(|| backend.strip_prefix("python:"))
    {
        return map_pypi(pkg, tool_name, backend, version);
    }

    if let Some(crate_name) = backend.strip_prefix("cargo:") {
        let name = crate_name.trim();
        if name.is_empty() {
            return Err(PurlError::InvalidBackend {
                tool: tool_name.to_string(),
                backend: backend.to_string(),
                reason: "cargo crate name is empty".to_string(),
            });
        }
        return Ok(PackageUrl {
            pkg_type: "cargo".to_string(),
            namespace: None,
            name: name.to_string(),
            version: version.to_string(),
            qualifiers: Vec::new(),
            subpath: None,
        });
    }

    if let Some(repo) = backend
        .strip_prefix("aqua:")
        .or_else(|| backend.strip_prefix("github:"))
        .or_else(|| backend.strip_prefix("ubi:"))
    {
        return map_github(repo, tool_name, backend, version, download_url);
    }

    if let Some(module) = backend.strip_prefix("go:") {
        return map_golang(module, tool_name, backend, version);
    }

    let generic_name = backend
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(tool_name)
        .trim();
    if generic_name.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "tool name is empty".to_string(),
        });
    }

    let mut qualifiers = Vec::new();
    if let Some(url) = download_url.filter(|u| !u.trim().is_empty()) {
        qualifiers.push(("download_url".to_string(), url.trim().to_string()));
    }

    Ok(PackageUrl {
        pkg_type: "generic".to_string(),
        namespace: None,
        name: percent_encode(generic_name),
        version: version.to_string(),
        qualifiers,
        subpath: None,
    })
}

fn map_npm(
    pkg: &str,
    tool_name: &str,
    backend: &str,
    version: &str,
) -> Result<PackageUrl, PurlError> {
    let pkg = pkg.trim();
    if pkg.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "npm package name is empty".to_string(),
        });
    }

    if let Some(scoped) = pkg.strip_prefix('@') {
        let Some((scope, name)) = scoped.split_once('/') else {
            return Err(PurlError::InvalidBackend {
                tool: tool_name.to_string(),
                backend: backend.to_string(),
                reason: "scoped npm package must be in `@scope/name` format".to_string(),
            });
        };
        if scope.is_empty() || name.is_empty() || name.contains('/') {
            return Err(PurlError::InvalidBackend {
                tool: tool_name.to_string(),
                backend: backend.to_string(),
                reason: "invalid scoped npm package name".to_string(),
            });
        }
        return Ok(PackageUrl {
            pkg_type: "npm".to_string(),
            namespace: Some(format!("%40{}", scope.to_ascii_lowercase())),
            name: name.to_ascii_lowercase(),
            version: version.to_string(),
            qualifiers: Vec::new(),
            subpath: None,
        });
    }

    if pkg.contains('/') {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "unscoped npm package must not contain `/`".to_string(),
        });
    }

    Ok(PackageUrl {
        pkg_type: "npm".to_string(),
        namespace: None,
        name: pkg.to_ascii_lowercase(),
        version: version.to_string(),
        qualifiers: Vec::new(),
        subpath: None,
    })
}

fn map_pypi(
    pkg: &str,
    tool_name: &str,
    backend: &str,
    version: &str,
) -> Result<PackageUrl, PurlError> {
    let pkg = pkg.trim();
    if pkg.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "pypi package name is empty".to_string(),
        });
    }

    let normalized = pkg.to_ascii_lowercase().replace('_', "-");
    Ok(PackageUrl {
        pkg_type: "pypi".to_string(),
        namespace: None,
        name: normalized,
        version: version.to_string(),
        qualifiers: Vec::new(),
        subpath: None,
    })
}

fn map_github(
    repo: &str,
    tool_name: &str,
    backend: &str,
    version: &str,
    download_url: Option<&str>,
) -> Result<PackageUrl, PurlError> {
    let repo = repo.trim();
    if repo.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "repository slug is empty".to_string(),
        });
    }

    // `ubi:` can occasionally reference an arbitrary URL rather than `owner/repo`.
    if repo.starts_with("http://") || repo.starts_with("https://") {
        let url = download_url.unwrap_or(repo);
        return Ok(PackageUrl {
            pkg_type: "generic".to_string(),
            namespace: None,
            name: percent_encode(tool_name),
            version: version.to_string(),
            qualifiers: vec![("download_url".to_string(), url.to_string())],
            subpath: None,
        });
    }

    let mut parts = repo.splitn(3, '/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let subpath = parts.next().filter(|s| !s.is_empty()).map(str::to_string);

    if owner.is_empty() || name.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "expected `owner/repo` format".to_string(),
        });
    }

    Ok(PackageUrl {
        pkg_type: "github".to_string(),
        namespace: Some(owner.to_ascii_lowercase()),
        name: name.to_ascii_lowercase(),
        version: version.to_string(),
        qualifiers: Vec::new(),
        subpath,
    })
}

fn map_golang(
    module: &str,
    tool_name: &str,
    backend: &str,
    version: &str,
) -> Result<PackageUrl, PurlError> {
    let module = module.trim().trim_matches('/');
    if module.is_empty() {
        return Err(PurlError::InvalidBackend {
            tool: tool_name.to_string(),
            backend: backend.to_string(),
            reason: "go module path is empty".to_string(),
        });
    }

    let (namespace, name) = match module.rsplit_once('/') {
        Some((ns, n)) if !ns.is_empty() && !n.is_empty() => (Some(ns.to_string()), n.to_string()),
        _ => (None, module.to_string()),
    };

    Ok(PackageUrl {
        pkg_type: "golang".to_string(),
        namespace,
        name,
        version: version.to_string(),
        qualifiers: Vec::new(),
        subpath: None,
    })
}

/// Percent-encodes a string according to RFC 3986 unreserved characters (`A-Za-z0-9-._~`).
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte & 0x0f));
        }
    }
    out
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_supported_backends_to_purls() {
        assert_eq!(
            backend_to_purl("npm:prettier", "npm:prettier", "3.9.10", None)
                .unwrap()
                .to_string(),
            "pkg:npm/prettier@3.9.10"
        );
        assert_eq!(
            backend_to_purl("npm:@commitlint/cli", "npm:@commitlint/cli", "21.2.4", None)
                .unwrap()
                .to_string(),
            "pkg:npm/%40commitlint/cli@21.2.4"
        );
        assert_eq!(
            backend_to_purl("pipx:black", "black", "25.1.0", None)
                .unwrap()
                .to_string(),
            "pkg:pypi/black@25.1.0"
        );
        assert_eq!(
            backend_to_purl("pipx:my_tool", "my_tool", "1.2.3", None)
                .unwrap()
                .to_string(),
            "pkg:pypi/my-tool@1.2.3"
        );
        assert_eq!(
            backend_to_purl("cargo:ripgrep", "ripgrep", "14.1.1", None)
                .unwrap()
                .to_string(),
            "pkg:cargo/ripgrep@14.1.1"
        );
        assert_eq!(
            backend_to_purl("aqua:evilmartians/lefthook", "lefthook", "2.2.2", None)
                .unwrap()
                .to_string(),
            "pkg:github/evilmartians/lefthook@2.2.2"
        );
        assert_eq!(
            backend_to_purl("github:cli/cli", "gh", "2.67.0", None)
                .unwrap()
                .to_string(),
            "pkg:github/cli/cli@2.67.0"
        );
        assert_eq!(
            backend_to_purl("ubi:sharkdp/bat", "bat", "0.25.0", None)
                .unwrap()
                .to_string(),
            "pkg:github/sharkdp/bat@0.25.0"
        );
        assert_eq!(
            backend_to_purl(
                "core:node",
                "node",
                "26.13.1",
                Some("https://nodejs.org/dist/v26.13.1/node-v26.13.1-linux-x64.tar.gz")
            )
            .unwrap()
            .to_string(),
            "pkg:generic/node@26.13.1?download_url=https%3A%2F%2Fnodejs.org%2Fdist%2Fv26.13.1%2Fnode-v26.13.1-linux-x64.tar.gz"
        );
        assert_eq!(
            backend_to_purl("core:rust", "rust", "1.100.0", None)
                .unwrap()
                .to_string(),
            "pkg:generic/rust@1.100.0"
        );
    }

    #[test]
    fn maps_aqua_subpath() {
        assert_eq!(
            backend_to_purl(
                "aqua:kubernetes/kubernetes/kubectl",
                "kubectl",
                "1.32.0",
                None
            )
            .unwrap()
            .to_string(),
            "pkg:github/kubernetes/kubernetes@1.32.0#kubectl"
        );
    }
}
