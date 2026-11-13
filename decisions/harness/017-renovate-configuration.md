# 017: Renovate for automated dependency updates

- **Status:** Active
- **Finality:** Standard
- **Scope:** `renovate.json`, `agent-context/harness/pull-requests.md`,
  `agent-context/harness/task-queue.md`
- **Date:** 2026-11-13

## Context

Tool versions in `mise.toml`, GitHub Actions in `.github/workflows/`, and
project dependencies need automated update PRs that comply with `commitlint` and
the repository's label taxonomy.

## Decision

Use Renovate (`renovate.json`) extending `config:best-practices` with `chore`
conventional commits and labels, `ignorePaths: ["**/.mise/**"]` so internal lock
directories are not scanned as standalone npm packages, a 3-day
`minimumReleaseAge` cooldown for non-major updates, and grouped GitHub Actions
updates.

## Options Considered

- **Mend Renovate (chosen):** Natively understands `mise.toml`, GitHub Actions,
  and language package managers, supports digest pinning and release-age
  cooldowns.
- **Dependabot:** Rejected because Dependabot does not natively support
  `mise.toml` tool pinning.

## Consequences

Dependency and tool updates are opened automatically with digest pinning,
up-to-date `mise.lock` / `.mise/locks/` entries, and compliant commit messages.
Passing Renovate PRs are ready for human merge directly, while agents only check
out Renovate branches when CI fails (for example, formatting drift after a
linter/formatter update or a new CVE caught by `mise run scan`).
