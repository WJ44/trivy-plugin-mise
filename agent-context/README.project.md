<!-- project:intro:start -->

# trivy-plugin-mise

Trivy plugin and standalone CLI (written in Rust) that bridges
[mise](https://github.com/jdx/mise) (`mise.toml`, `mise.lock`, `.mise/locks/`)
with [Trivy](https://github.com/aquasecurity/trivy) for CycloneDX SBOM
generation and vulnerability scanning.

<!-- project:intro:end -->

<!-- project:sections:start -->

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
"github:WJ44/trivy-plugin-mise" = "0.1.0"
```

Then run `mise install` (and `mise lock` to pin platform checksums) and invoke
`trivy-plugin-mise` directly:

```bash
trivy-plugin-mise --help
```

## CLI Usage

```bash
cargo run -- --help
cargo run -- --version
cargo run -- sbom --help
cargo run -- scan --help
cargo test
```

<!-- project:sections:end -->
