# trivy-plugin-mise

Trivy plugin and standalone CLI (written in Rust) that bridges
[mise](https://github.com/jdx/mise) (`mise.toml`, `mise.lock`, `.mise/locks/`)
with [Trivy](https://github.com/aquasecurity/trivy) for CycloneDX SBOM
generation and vulnerability scanning.

## Architecture & Distribution

- **Implementation language:** Rust (see
  [Decision 001](decisions/001-language-and-distribution.md)).
- **Distribution:** Shipped as a single static `trivy-plugin-mise` binary with a
  root `plugin.yaml` manifest, supporting both
  `trivy plugin install github.com/WJ44/trivy-plugin-mise` (`trivy mise ...`)
  and direct installation via `mise` (`github:WJ44/trivy-plugin-mise`).

## Installation

### As a Trivy plugin

Install directly from GitHub via `trivy plugin install`:

```bash
trivy plugin install github.com/WJ44/trivy-plugin-mise
trivy mise --help
```

### Standalone CLI via `mise`

Pin in `mise.toml` using the `github:` backend:

```toml
[tools]
"github:WJ44/trivy-plugin-mise" = "1.0.0"
```

Then run `mise install` (and `mise lock` to pin platform checksums) and invoke
`trivy-plugin-mise` directly:

```bash
trivy-plugin-mise --help
```

## CLI Usage

Running `trivy mise` (or `trivy-plugin-mise`) with no subcommand executes `scan`
by default, matching standard Trivy plugin ergonomics:

```bash
# Default scan (equivalent to `trivy mise scan .`)
trivy mise
trivy mise /path/to/repo -c trivy.yaml -s HIGH,CRITICAL -q --ignore-unfixed
trivy mise --scanners vuln -- --skip-db-update --offline-scan

# Explicit subcommands
trivy mise scan --help
trivy mise sbom --output sbom.cdx.json -- --skip-dirs vendor
trivy mise version
```

- **Ignore file auto-discovery:** When `--ignorefile` is omitted, `scan` checks
  `<target>/.trivyignore.yaml` first and falls back to `<target>/.trivyignore`
  if present.
- **Clean output streams:** Tier progress headers (`==> ...`) and
  `==> Scan summary: ...` are written to `stderr` so `stdout` stays clean for
  piped or machine-readable Trivy output, and are suppressed when `-q`/`--quiet`
  is set.

### Development

```bash
cargo run -- --help
cargo run -- --version
cargo run -- sbom --help
cargo run -- scan --help
cargo test
```

## Common Commands

### Formatting, Linting & Security

| Command                     | Description                                                 |
| --------------------------- | ----------------------------------------------------------- |
| `lefthook run format`       | Auto-format all files (`--file <path>` for specific files)  |
| `lefthook run format-check` | Check formatting without modifying files                    |
| `lefthook run lint`         | Lint and auto-fix all files where supported                 |
| `lefthook run lint-check`   | Check for lint issues without modifying files               |
| `mise run sbom`             | Generate CycloneDX SBOM (`sbom.cdx.json`)                   |
| `mise run scan`             | Scan dependencies for CVEs and license compliance via Trivy |
