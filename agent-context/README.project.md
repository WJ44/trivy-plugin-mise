<!-- project:intro:start -->

# trivy-plugin-mise

Trivy plugin and standalone CLI (written in Rust) that bridges
[mise](https://github.com/jdx/mise) (`mise.toml`, `mise.lock`, `.mise/locks/`)
with [Trivy](https://github.com/aquasecurity/trivy) for CycloneDX SBOM
generation and vulnerability scanning.

<!-- project:intro:end -->

<!-- project:sections:start -->

## Features

- **CycloneDX 1.7 SBOM Generation (`sbom`):**
  - **`--mode formulation` (default):** Runs `trivy fs` on the repository root
    for shipped product dependencies (`components[]`) and injects `mise`-managed
    development and build tools (`mise.lock` and `.mise/locks/`) into CycloneDX
    1.7 `formulation[]` (Manufacturing BOM / MBOM) with workflow metadata and a
    preserved dependency graph.
  - **`--mode components`:** Emits a standalone CycloneDX 1.7 SBOM listing only
    the `mise` toolchain and transitive sidecar dependencies in top-level
    `components[]`.
- **`mise.lock` & `.mise/locks/` Sidecar Extraction:** Normalizes `mise.lock`
  (`lockfile_version = 3`) backends (`npm`, `pipx`/`python`, `cargo`,
  `aqua`/`github`/`ubi`, `go`, and generic tools) into standard Package URLs
  (PURLs) with platform checksums, download URLs, and provenance properties.
  Verifies SHA-256 digests and materializes `.mise/locks/<tool>/<version>/`
  sidecar lockfiles (`aube-lock.yaml` mapped to `pnpm-lock.yaml`, `uv.lock`,
  `Cargo.lock`, etc.) in an ephemeral workspace for Trivy inspection without
  mutating the repository.
- **Multi-Tier Vulnerability & License Scanning (`scan`):**
  1. **Repository tier (`repo`):** Scans repository dependencies for
     vulnerabilities and license compliance (`--scanners vuln,license`),
     skipping `.mise/locks/`.
  2. **Toolchain sidecar tier (`toolchain`):** Verifies and materializes
     `.mise/locks/` sidecar lockfiles and scans transitive tool dependencies for
     vulnerabilities (`--scanners vuln`), avoiding copyleft license
     false-positives on unshipped developer tools.
  3. **Active binary tier (`gobinary`, opt-in via `--include-gobinary`):**
     Discovers active `mise` tool install directories (`mise ls --json`) and
     scans compiled binaries via `trivy rootfs --scanners vuln`.
- **Dual Distribution:** Shipped as a single static `trivy-plugin-mise` binary
  with a root `plugin.yaml` manifest (see
  [Decision 001](decisions/001-language-and-distribution.md)), supporting both
  `trivy plugin install` (`trivy mise ...`) and standalone installation via
  `mise` (`trivy-plugin-mise ...`).

For full CLI flags, PURL mappings, sidecar lockfile mechanics, and CI recipes,
see the [**Usage Guide (`docs/usage.md`)**](docs/usage.md).

## Installation

`trivy-plugin-mise` requires [`trivy`](https://github.com/aquasecurity/trivy) on
`PATH` (and [`mise`](https://github.com/jdx/mise) on `PATH` when using
`scan --include-gobinary`).

### As a Trivy plugin

Install directly from GitHub via `trivy plugin install`:

```bash
trivy plugin install github.com/WJ44/trivy-plugin-mise
trivy mise --help
```

### Standalone CLI via `mise`

Pin in `mise.toml` using the `github:` backend (alongside `trivy`):

```toml
[tools]
trivy = "0.76.2"
"github:WJ44/trivy-plugin-mise" = "1.1.0"
```

Then run `mise install` (and `mise lock` to pin platform checksums) and invoke
`trivy-plugin-mise` directly:

```bash
trivy-plugin-mise --help
```

## Quickstart

### Generate a CycloneDX 1.7 SBOM

Generate a product SBOM with the `mise` toolchain recorded under `formulation[]`
(default `--mode formulation`):

```bash
# Via Trivy plugin
trivy mise sbom --output sbom.cdx.json

# Or via standalone CLI
trivy-plugin-mise sbom --output sbom.cdx.json
```

Generate a standalone toolchain SBOM (`--mode components`) pinned to a target
platform triple:

```bash
trivy mise sbom --mode components --platform linux-x64 --output sbom-tools.cdx.json
# Or:
trivy-plugin-mise sbom --mode components --platform linux-x64 --output sbom-tools.cdx.json
```

### Scan for Vulnerabilities & Licenses

Run the default two-tier scan (repository `vuln,license` + `.mise/locks/`
toolchain `vuln`), automatically using `<target>/trivy.yaml` and
`<target>/.trivyignore.yaml` when present:

```bash
# Via Trivy plugin
trivy mise scan

# Or via standalone CLI
trivy-plugin-mise scan
```

Include active `mise` tool install directories (`trivy rootfs --scanners vuln`)
or scope to a specific tier:

```bash
trivy mise scan --include-gobinary
trivy-plugin-mise scan --repo-only
trivy-plugin-mise scan --toolchain-only --severity HIGH,CRITICAL
```

See [`docs/usage.md`](docs/usage.md) for complete documentation.

<!-- project:sections:end -->
