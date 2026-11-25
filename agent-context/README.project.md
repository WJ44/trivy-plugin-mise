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
    development and build tools (across standard layered configs/lockfiles and
    sidecar `locks/` directories) into CycloneDX 1.7 `formulation[]`
    (Manufacturing BOM / MBOM) with workflow metadata and a preserved dependency
    graph.
  - **`--mode components`:** Emits a standalone CycloneDX 1.7 SBOM listing only
    the `mise` toolchain and transitive sidecar dependencies in top-level
    `components[]`.
- **Layered `mise` Discovery & Stale-Entry Filtering:** Pure-filesystem
  discovery of standard repository-scoped `mise` configuration files
  (`.config/mise/conf.d/*.toml`, `.config/mise/config.toml`,
  `.config/mise/mise.toml`, `.config/mise.toml`, `.mise/conf.d/*.toml`,
  `.mise/config.toml`, `mise/conf.d/*.toml`, `mise/config.toml`, `mise.toml`,
  `.mise.toml`), their lockfiles (`.config/mise/mise.lock`, `.config/mise.lock`,
  `.mise/mise.lock`, `mise/mise.lock`, `mise.lock`), and sidecar directories
  (`.config/mise/locks/`, `.mise/locks/`, `mise/locks/`). Machine-local
  `*.local.toml` / `*.local.lock` overrides are excluded by default and opt-in
  via `--include-local`; non-standard paths can be configured via repeatable
  `--mise-config <PATH>` and `--mise-lock <PATH>` flags. When active `mise`
  config files exist, unconfigured/stale lockfile entries (and their sidecars)
  are warned on `stderr` and omitted, while lockfile-only targets continue to
  work cleanly.
- **`mise.lock` & Sidecar Lockfile Extraction:** Normalizes `mise.lock`
  (`lockfile_version = 3` and `4`) backends (`npm`, `pipx`/`python`, `cargo`,
  `aqua`/`github`/`ubi`, `go`, and generic tools) into standard Package URLs
  (PURLs) with platform checksums, download URLs, and provenance properties.
  Verifies SHA-256 digests and materializes sidecar lockfiles (`aube-lock.yaml`
  mapped to `pnpm-lock.yaml`, `uv.lock`, `Cargo.lock`, etc.) in an ephemeral
  workspace for Trivy inspection without mutating the repository.
- **Multi-Tier Vulnerability & License Scanning (`scan`):**
  1. **Repository tier (`repo`):** Scans repository dependencies for
     vulnerabilities and license compliance (`--scanners vuln,license`),
     skipping all discovered sidecar lock directories (`.config/mise/locks/`,
     `.mise/locks/`, `mise/locks/`, etc.).
  2. **Toolchain sidecar tier (`toolchain`):** Verifies and materializes active
     sidecar lockfiles and scans transitive tool dependencies for
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

Install via the
[`trivy-plugin-index`](https://github.com/aquasecurity/trivy-plugin-index)
shorthand (or directly from the GitHub repository URL):

```bash
# Refresh the plugin index, search, and install by name (or pinned version)
trivy plugin update
trivy plugin search mise
trivy plugin install mise
# Or install a specific version:
trivy plugin install mise@v1.3.0 # x-release-please-version

# Or install directly from GitHub without the plugin index:
trivy plugin install github.com/WJ44/trivy-plugin-mise

# Upgrade an existing installation or inspect help:
trivy plugin upgrade mise
trivy mise --help
```

### Standalone CLI via `mise`

Pin in `mise.toml` using the `github:` backend (alongside `trivy`):

```toml
[tools]
trivy = "0.76.2"
"github:WJ44/trivy-plugin-mise" = "1.3.0" # x-release-please-version
```

Then run `mise install` (and `mise lock` to pin platform checksums) and invoke
`trivy-plugin-mise` directly:

```bash
trivy-plugin-mise --help
```

## Quickstart

### Scan for Vulnerabilities & Licenses

Running `trivy mise` (or `trivy-plugin-mise`) with no subcommand executes `scan`
by default, matching standard Trivy plugin ergonomics. It runs the two-tier scan
(repository `vuln,license` + `mise` toolchain sidecar `vuln`), automatically
discovering `<target>/trivy.yaml` and `<target>/.trivyignore.yaml` (or falling
back to `<target>/.trivyignore`):

```bash
# Default scan (equivalent to `trivy mise scan .`)
trivy mise
trivy-plugin-mise

# Custom config, severity filter, quiet mode, ignore-unfixed, scanners, and raw Trivy args
trivy mise /path/to/repo -c trivy.yaml -s HIGH,CRITICAL -q --ignore-unfixed
trivy mise --scanners vuln -- --skip-db-update --offline-scan

# Include active `mise` tool install directories (`trivy rootfs --scanners vuln`) or scope to a tier
trivy mise scan --include-gobinary
trivy-plugin-mise scan --repo-only
trivy-plugin-mise scan --toolchain-only --severity HIGH,CRITICAL

# Include machine-local overrides (`*.local.toml` / `*.local.lock`) or pass custom paths
trivy-plugin-mise scan --include-local
trivy-plugin-mise scan --mise-config custom/mise.toml --mise-lock custom/mise.lock
```

- **Ignore file auto-discovery:** When `--ignorefile` is omitted, `scan` checks
  `<target>/.trivyignore.yaml` first and falls back to `<target>/.trivyignore`
  if present.
- **Clean output streams:** Tier progress headers (`==> ...`) and
  `==> Scan summary: ...` are written to `stderr` so `stdout` stays clean for
  piped or machine-readable Trivy output, and are suppressed when `-q`/`--quiet`
  is set.

### Generate a CycloneDX 1.7 SBOM

Generate a product SBOM with the `mise` toolchain recorded under `formulation[]`
(default `--mode formulation`), optionally forwarding extra flags to `trivy fs`
after `--`:

```bash
# Via Trivy plugin
trivy mise sbom --output sbom.cdx.json
trivy mise sbom --output sbom.cdx.json -- --skip-dirs vendor

# Or via standalone CLI
trivy-plugin-mise sbom --output sbom.cdx.json
```

Generate a standalone toolchain SBOM (`--mode components`) pinned to a target
platform triple:

```bash
trivy mise sbom --mode components --platform linux-x64 --output sbom-tools.cdx.json
# Include machine-local overrides or specify custom config/lockfile paths:
trivy-plugin-mise sbom --include-local --output sbom.cdx.json
trivy-plugin-mise sbom --mise-config custom/mise.toml --mise-lock custom/mise.lock --output sbom.cdx.json
```

See [`docs/usage.md`](docs/usage.md) for complete documentation.

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
  ([http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
- [MIT license](LICENSE-MIT)
  ([http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

<!-- project:sections:end -->
