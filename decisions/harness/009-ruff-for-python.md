# 009: Ruff for Python

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, `*.py`
- **Date:** 2026-11-08

## Context

Python scripts and code require fast static analysis and linting without
requiring a dedicated Python virtual environment just to run the linter.

## Decision

Use `ruff` to lint Python files.

## Options Considered

- **`ruff` (chosen):** Standalone Rust binary installable directly via `mise`,
  fast, supports autofix (`--fix`), and replaces multiple legacy Python linters.
- **`flake8` / `pylint` / `isort`:** Rejected because they are slower, require
  Python environment management, and split linting across multiple tools.

## Consequences

Python files are linted automatically without needing a `venv`. Revisit adding
`ruff format` or a dedicated type checker (`ty`, `pyright`, `mypy`) when a
project's Python codebase grows beyond standalone scripts.
