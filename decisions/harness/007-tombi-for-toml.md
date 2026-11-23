# 007: tombi for TOML files

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, `*.toml`
- **Date:** 2026-11-23

## Context

Prettier does not format TOML out of the box, and TOML files also benefit from
syntax, JSON Schema, and project-extension validation (`mise.toml`,
`.rumdl.toml`, `pyproject.toml`, `Cargo.toml`, `tombi.toml`). `taplo` and
`tombi` are the main standalone TOML formatters/linters, and `taplo` is largely
unmaintained.

## Decision

Use `tombi` to both format (`tombi format`) and lint (`tombi lint`) TOML files,
keeping its default exit behavior (logged errors fail; remote SchemaStore fetch
failures log warnings and degrade gracefully without `--error-on-warnings`).

## Options Considered

- **`tombi` for formatting and linting (chosen):** Actively maintained
  Rust-based TOML tool providing schema-aware formatting, key ordering, and JSON
  Schema / extension linting in one binary, with default warning tolerance
  preserving offline and sandboxed runs.
- **`tombi` formatting only (linting deferred):** Previously used before `tombi`
  expanded schema/extension lint rules and CLI error exit-code handling;
  revisited so TOML schema and syntax mistakes fail fast in hooks and CI.
- **`--error-on-warnings` enabled globally:** Rejected because transient or
  sandbox-blocked remote SchemaStore fetches log warnings that should degrade
  gracefully rather than break CI.
- **`taplo`:** Considered, but rejected because upstream maintenance has
  stalled.

## Consequences

TOML files are formatted consistently with schema-aware key ordering and
validated against SchemaStore schemas and built-in extension rules. Because
`tombi` fetches JSON Schemas from `www.schemastore.org` (cached locally), it
requires network allowlist access to `www.schemastore.org` in sandboxed
environments, and its formatting or lint diagnostics can change when an upstream
schema changes.
