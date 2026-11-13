# 006: Tool installation with mise

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `mise.lock`
- **Date:** 2026-11-03

## Context

Formatters, linters, and workflow tools need to be installed and version-pinned
identically in local environments and in CI without polluting the repository
root with language-specific manifests (such as a root `package.json` just to
install Prettier or `commitlint`).

## Decision

Manage and pin development tools and runtimes in `mise.toml` with checksums
locked in `mise.lock`.

## Options Considered

- **`mise` with `mise.toml` and `mise.lock` (chosen):** Language-agnostic single
  manifest for runtimes, Go/Rust binaries, and `npm:` CLI tools; keeps dev
  tooling isolated from application dependency manifests and runs identically in
  CI via `jdx/mise-action`.
- **Root `package.json` + system `apt` packages:** Rejected because a root
  `package.json` collides with non-JS or multi-package projects, and `apt`
  cannot pin consistent cross-platform tool versions.

## Consequences

Tool versions are pinned in a single language-agnostic file (`mise.toml`), and
local environments and CI install the exact same locked versions without
touching application manifests.
