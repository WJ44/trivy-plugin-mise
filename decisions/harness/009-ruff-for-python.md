# 009: Ruff for Python

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, `*.py`
- **Date:** 2026-11-18

## Context

Python scripts and code require fast static analysis, linting, and consistent
formatting without requiring a dedicated Python virtual environment just to run
checks.

## Decision

Use `ruff` to both lint (`ruff check`) and format (`ruff format`) Python files.

## Options Considered

- **`ruff` for linting and formatting (chosen):** Standalone Rust binary
  installable directly via `mise`, fast, supports autofix (`--fix`) and
  Black-compatible formatting with built-in defaults, and replaces multiple
  legacy Python tools (`flake8`, `isort`, `black`) in one dependency.
- **`ruff` linting only (formatting deferred):** Previously used when Python
  files were minimal; revisited once standalone scripts grew beyond trivial
  helpers to avoid manual style drift across `*.py` files.
- **`flake8` / `pylint` / `isort` / `black`:** Rejected because they are slower,
  require Python environment management, and split linting and formatting across
  multiple tools.

## Consequences

Python files are formatted and linted automatically without needing a `venv`.
Revisit adding a dedicated type checker (`ty`, `pyright`, `mypy`) when a
project's Python codebase grows beyond standalone scripts.
