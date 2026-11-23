# 015: GitHub Actions CI pipeline

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/workflows/ci.yml`
- **Date:** 2026-11-23

## Context

Quality checks (`format-check` and `lint-check`), harness script tests, and tool
versions are defined in `lefthook.yml`, `tests/run.py`, and `mise.toml` for
local execution and git hooks, and need to be enforced identically in CI on
pushes to `main` and on all pull requests (including stacked PRs targeting
unmerged parent branches).

## Decision

Run CI on GitHub Actions (`ubuntu-latest`), using `jdx/mise-action` to install
the pinned tools from `mise.toml` / `mise.lock` and invoking `lefthook` for
formatting and linting checks plus `mise run test` when `tests/run.py` is
present. Trigger `push` on `main` and `pull_request` across all base branches
(without a `branches: [main]` filter).

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
- **Unfiltered `pull_request` triggers with `push` scoped to `main` (chosen):**
  Ensures stacked PRs targeting non-`main` parent branches
  (`gh pr create --base <parent-branch>`) receive automated CI checks before
  downstream tasks stack on them, while avoiding duplicate CI runs on un-PRed
  branch pushes.
- **Restrict `pull_request` to `branches: [main]`:** Rejected because stacked
  PRs targeting unmerged parent branches skip CI checks and force task-queue
  agents to rely on local verification heuristics.

## Consequences

Every push to `main` and every pull request (including stacked PRs targeting
feature branches) is verified against the exact pinned formatters and linters
without duplicating tool configuration in CI workflows.
