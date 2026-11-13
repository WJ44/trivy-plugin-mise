# 001: Implementation language and distribution model

- **Status:** Active
- **Finality:** Standard
- **Scope:** Implementation language, Cargo/Rust toolchain (`mise.toml`,
  `lefthook.yml`), and release distribution (`plugin.yaml`, GitHub Releases)
- **Date:** 2026-11-13

## Context

`trivy-plugin-mise` bridges `jdx/mise` (`mise.toml`, `mise.lock`,
`.mise/locks/`) with `aquasecurity/trivy` for CycloneDX SBOM generation
(`formulation[]` and `components[]`) and vulnerability scanning.

Before implementing the core CLI, the project needs a foundational decision on:

1. **Implementation language** — must compile to small, self-contained,
   cross-platform static binaries (`linux-x64`, `linux-arm64`, `macos-x64`,
   `macos-arm64`, `windows-x64`) with zero runtime interpreter dependencies,
   fast/strict TOML, YAML, and JSON parsing/serialization (`mise.lock`,
   `aube-lock.yaml`, CycloneDX 1.7), and clean integration with `mise` and
   `lefthook`.
2. **Distribution model** — must work seamlessly both when invoked as
   `trivy mise ...` (via `trivy plugin install`) and when pinned directly in
   `mise.toml` (`"github:WJ44/trivy-plugin-mise"`) and invoked as
   `trivy-plugin-mise ...`.

## Decision

1. **Implementation language: Rust (`cargo`).** Use Rust as the implementation
   language, with `rustfmt` for formatting and `clippy` for linting integrated
   into `mise.toml` and `lefthook.yml`.
2. **Distribution model: Dual distribution from shared GitHub Release
   archives.** Ship a single static binary (`trivy-plugin-mise`) in GitHub
   Releases, paired with a root `plugin.yaml` manifest so both
   `trivy plugin install github.com/WJ44/trivy-plugin-mise` and `mise`
   (`github:WJ44/trivy-plugin-mise`) consume the same release artifacts.

## Options Considered

### Implementation Language

- **Rust (`cargo`):**
  - _Pros:_ Same ecosystem as `mise` and `aube`; `serde`, `toml`, `serde_yaml`,
    and `serde_json` provide strong, strict typing for lockfiles and CycloneDX
    SBOMs; `rustfmt` and `clippy` integrate cleanly with `lefthook`; maintainer
    has prior familiarity with Rust.
  - _Cons:_ Cross-compilation in CI requires slightly more setup (e.g.,
    `cargo-zigbuild` or `cross`) than Go.
  - _Outcome:_ **Chosen.**
- **Go (`golang`):**
  - _Pros:_ Same ecosystem as `aquasecurity/trivy`; trivial cross-compilation
    (`GOOS`/`GOARCH`); fast compilation; standard formatting and linting
    (`gofmt`, `golangci-lint`).
  - _Cons:_ Less overlap with `mise`'s Rust codebase; maintainer has no prior Go
    experience.
  - _Outcome:_ **Rejected.**

### Distribution Model

- **Dual distribution (Trivy plugin + standalone `mise`-installable binary):**
  - _Pros:_ Covers both `trivy mise ...` workflows and standalone
    `trivy-plugin-mise ...` usage in `mise.toml` without maintaining separate
    build pipelines.
  - _Outcome:_ **Chosen.**
- **Trivy plugin only / standalone CLI only:**
  - _Cons:_ Forces users into a single invocation model and complicates pinning
    in `mise.toml` or integrating with `trivy plugin install`.
  - _Outcome:_ **Rejected.**

## Consequences

- `mise.toml` pins the Rust toolchain (`rust`), and `lefthook.yml` runs
  `rustfmt` and `clippy` on `.rs` files.
- Future CI release workflows must build cross-platform static binaries for
  `linux-x64`, `linux-arm64`, `macos-x64`, `macos-arm64`, and `windows-x64` and
  publish a compatible `plugin.yaml`.
- Revisit only if Trivy plugin API changes require tighter Go library coupling
  that cannot be satisfied over CLI/JSON interfaces.
