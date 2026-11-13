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

<!-- project:sections:end -->
