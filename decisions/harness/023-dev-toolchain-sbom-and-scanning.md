# 023: Separate dev toolchain from shipped dependencies in SBOM and scanning

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `mise.lock`, `.mise/locks/`, `bin/mise-sbom`,
  `agent-context/harness/sbom-and-scanning.md`
- **Date:** 2026-11-13

## Context

Standard filesystem scanners (`trivy fs`) do not parse `mise.toml`, `mise.lock`,
or `.mise/locks/`, leaving `mise`-managed development tools omitted from SBOMs
and CVE scans. At the same time, treating `mise` dev tools the same as shipped
application dependencies causes three problems:

1. **SBOM accuracy:** Product SBOMs (`components[]`) should only list
   dependencies that ship with or link into the artifact, not build/dev tools
   like formatters and linters.
2. **False-positive license violations:** Standalone CLI tools (such as
   `shellcheck` under `GPL-3.0`) run as separate processes rather than linking
   into the product, so scanning them under product license rules triggers
   copyleft violations.
3. **Unactionable `gobinary` CVEs:** Scanning pre-compiled third-party Go CLI
   binaries (`shfmt`, `lefthook`, `trivy`) flags Go `stdlib` CVEs that
   downstream projects cannot patch until upstream cuts a new release.

## Decision

Keep `mise`-managed development tools separate from shipped dependencies (see
[021](021-software-bill-of-materials-and-vulnerability-scanning.md)), using
`bin/mise-sbom` as a prototype bridge prior to extracting a standalone
`trivy-plugin-mise` plugin:

1. **SBOM placement:** Record `mise` tools and their dependency graphs under
   CycloneDX `formulation[]` rather than top-level `components[]`.
2. **Vulnerability scanning:** Scan ecosystem-backed `mise` tool lockfiles
   (`.mise/locks/`) for CVEs (`vuln` scanner only), while excluding product
   license checks and default `gobinary` scans on pre-compiled upstream CLI
   binaries.

## Options Considered

- **CycloneDX `formulation[]` + CVE-only scanning on `.mise/locks/` via
  `bin/mise-sbom` (chosen):** Records the full development toolchain and catches
  CVEs in ecosystem-backed dev tools without polluting shipped product
  `components[]` or failing on standalone CLI licenses and unactionable upstream
  Go `stdlib` binaries.
- **Listing `mise` tools in top-level `components[]`:** Rejected because it
  falsely claims downstream products ship dev linters/formatters and triggers
  copyleft license violations on standalone CLI tools.
- **Scanning `~/.local/share/mise/installs` with `gobinary` by default:**
  Rejected as a default gate because pre-compiled third-party Go binaries flag
  upstream Go `stdlib` CVEs that cannot be patched locally.

## Consequences

Shipped product dependencies stay isolated in `components[]` (scanned for both
CVEs and licenses), while `mise` dev tools are recorded in `formulation[]` and
their ecosystem lockfiles are scanned for CVEs. Once `trivy-plugin-mise` is
released, `bin/mise-sbom` can be replaced with the standalone plugin.
