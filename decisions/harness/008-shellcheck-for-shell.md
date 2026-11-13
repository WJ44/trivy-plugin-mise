# 008: ShellCheck for shell scripts

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, shell scripts
- **Date:** 2026-11-08

## Context

Shell scripts require static analysis to catch quoting bugs, uninitialized
variables, and portability pitfalls.

## Decision

Use `shellcheck` to lint shell scripts.

## Options Considered

- **`shellcheck` (chosen):** De facto standard static analyzer for POSIX and
  Bash shell scripts. No alternative shell linter was evaluated.

## Consequences

Shell scripts (including extensionless scripts discovered via shebang) are
statically analyzed on pre-commit, post-edit hooks, and in CI.
