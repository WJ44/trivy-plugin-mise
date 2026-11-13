# 004: shfmt for shell files

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, shell scripts
- **Date:** 2026-11-03

## Context

Prettier does not natively format shell scripts, so shell scripts need a
dedicated formatter.

## Decision

Use `shfmt` to format shell scripts.

## Options Considered

- **`shfmt` (chosen):** De facto standard shell formatter, shipped as a single
  static binary that reads `.editorconfig` and discovers extensionless shell
  scripts by shebang.
- **`prettier-plugin-sh`:** Rejected because it wraps `shfmt` inside Prettier
  via WASM rather than running `shfmt` directly.

## Consequences

Shell scripts across the repository (including extensionless scripts) are
formatted consistently by `shfmt`.
