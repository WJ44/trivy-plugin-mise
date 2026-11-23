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

## CLI Usage

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
