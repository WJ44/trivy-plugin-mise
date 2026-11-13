# 015: GitHub Actions CI pipeline

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/workflows/ci.yml`
- **Date:** 2026-11-08

## Context

Quality checks (`format-check` and `lint-check`) and tool versions are defined
in `lefthook.yml` and `mise.toml` for local execution and git hooks, and need to
be enforced identically in CI on pushes and pull requests to `main`.

## Decision

Run CI on GitHub Actions (`ubuntu-latest`), using `jdx/mise-action` to install
the pinned tools from `mise.toml` / `mise.lock` and invoking `lefthook` for
formatting and linting checks.

## Options Considered

- **`jdx/mise-action` + `lefthook` on `ubuntu-latest` (chosen):** Reuses the
  exact tool versions and entry points from local runs and git hooks (see
  [005](005-lefthook-runner.md) and [006](006-tool-installation.md)). Because
  tooling is hermetically pinned by `mise`, floating `ubuntu-latest` keeps the
  runner OS patched without manual runner-version maintenance.
- **Separate GitHub Actions per linter/formatter:** Rejected because third-party
  tool actions can drift from `mise.toml` versions and duplicate file-routing
  logic in `lefthook.yml`.
- **Pinned runner versions (e.g. `ubuntu-24.04`):** Rejected because pinning the
  runner OS adds deprecation maintenance overhead without benefit when tools are
  already pinned via `mise`.

## Consequences

Every push and pull request to `main` is verified against the exact pinned
formatters and linters without duplicating tool configuration in CI workflows.
