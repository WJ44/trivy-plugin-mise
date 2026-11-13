# 007: tombi for TOML files

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `lefthook.yml`, `*.toml`
- **Date:** 2026-11-03

## Context

Prettier does not format TOML out of the box. `taplo` and `tombi` are the main
standalone TOML formatters/linters, and `taplo` is largely unmaintained.

## Decision

Use `tombi` to format TOML files.

## Options Considered

- **`tombi` (chosen):** Actively maintained Rust-based TOML formatter with
  built-in SchemaStore validation and key ordering.
- **`taplo`:** Considered, but rejected because upstream maintenance has
  stalled.

## Consequences

TOML files are formatted consistently with schema-aware key ordering. Because
`tombi` fetches JSON Schemas from `www.schemastore.org` (cached locally), it
requires network allowlist access to `www.schemastore.org` in sandboxed
environments, and its formatting output can change when an upstream schema
changes.
