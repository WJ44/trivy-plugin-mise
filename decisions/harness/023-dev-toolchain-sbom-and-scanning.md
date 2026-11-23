# 023: Separate dev toolchain from shipped dependencies in SBOM and scanning

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `mise.lock`, `.mise/locks/`, `bin/mise-sbom`,
  `agent-context/harness/sbom-and-scanning.md`
- **Date:** 2026-11-13

## Context

Standard filesystem scanners miss `mise`-managed development tools, leaving the
dev toolchain absent from SBOMs and vulnerability scans. At the same time,
treating `mise` dev tools identically to shipped application dependencies causes
three problems:

1. **SBOM accuracy:** Product component inventories should list only
   dependencies that ship with or link into the deliverable, not build/dev tools
   such as formatters and linters.
2. **False-positive license violations:** Standalone CLI tools (such as
   `shellcheck` under `GPL-3.0`) run as separate processes rather than linking
   into the product, so evaluating them under shipped-product license rules
   triggers false copyleft violations.
3. **Unactionable runtime CVEs:** Scanning pre-compiled third-party Go CLI
   binaries (`shfmt`, `lefthook`, `trivy`) flags upstream Go standard-library
   CVEs that downstream projects cannot patch until upstream cuts a new release.

## Decision

Keep `mise`-managed development tools separate from shipped dependencies (see
[021](021-software-bill-of-materials-and-vulnerability-scanning.md)), using
`bin/mise-sbom` as a prototype bridge prior to extracting a standalone
`trivy-plugin-mise` plugin:

1. **SBOM placement:** Record `mise` tools and their dependency graphs as
   build/formulation metadata in the CycloneDX SBOM rather than top-level
   shipped product components.
2. **Vulnerability scanning:** Scan ecosystem-backed `mise` tool lockfiles for
   vulnerabilities only, excluding shipped-product license checks and binary
   runtime scans on pre-compiled upstream CLI binaries.

## Options Considered

- **Separate SBOM formulation + lockfile-based vulnerability scanning via
  `bin/mise-sbom` (chosen):** Records the full development toolchain and catches
  CVEs in ecosystem-backed dev tools without polluting shipped product
  components or failing on standalone CLI licenses and unactionable upstream Go
  binaries.
- **Listing `mise` tools as shipped product components:** Rejected because it
  falsely claims downstream products ship dev linters/formatters and triggers
  copyleft license violations on standalone CLI tools.
- **Scanning installed pre-compiled tool binaries by default:** Rejected as a
  default gate because pre-compiled third-party Go binaries flag upstream
  standard-library CVEs that cannot be patched locally.

## Consequences

Shipped product dependencies stay isolated in the main SBOM component list
(scanned for both CVEs and licenses), while `mise` dev tools are recorded
separately as formulation metadata and their ecosystem lockfiles are scanned for
CVEs. Once `trivy-plugin-mise` is released, `bin/mise-sbom` can be replaced with
the standalone plugin.
