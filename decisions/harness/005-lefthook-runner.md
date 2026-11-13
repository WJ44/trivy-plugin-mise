# 005: lefthook as hook and task runner

- **Status:** Active
- **Finality:** Standard
- **Scope:** `lefthook.yml`, git hooks (`pre-commit`, `commit-msg`)
- **Date:** 2026-11-03

## Context

With multiple formatters and linters in the repository, changed or tracked files
must be routed to the appropriate tool across manual runs, git hooks, CI, and
agent hooks.

## Decision

Use `lefthook` as the git hook and quality-check runner (`format`,
`format-check`, `lint`, `lint-check`).

## Options Considered

- **`lefthook` (chosen):** Fast single binary installable via `mise`, supports
  both git hooks and standalone CLI task execution, and re-stages formatted
  files on `pre-commit` natively.
- **Custom shell wrapper scripts / `pre-commit` (Python) / `husky` (Node):**
  Rejected to avoid tying the hook runner to a Python virtualenv or a root
  `package.json`.

## Consequences

Adding a language formatter or linter only requires adding a job to
`lefthook.yml`, and manual runs, git hooks, CI, and agent hooks all invoke the
same commands.
