# 010: Markdownlint evaluation

- **Status:** Active
- **Finality:** Provisional
- **Scope:** `*.md`, `lefthook.yml`, `mise.toml`
- **Date:** 2026-11-08

## Context

Evaluated whether to add `markdownlint-cli2` for linting Markdown documentation
alongside Prettier.

## Decision

Do not add `markdownlint-cli2` at this time; rely on Prettier for Markdown
formatting and consistency.

## Options Considered

- **Prettier only, no `markdownlint-cli2` (chosen):** Prettier already formats
  prose wrapping, tables, lists, and code blocks; skipping `markdownlint-cli2`
  avoids duplicate tooling and rule conflicts.
- **`markdownlint-cli2`:** Evaluated, but its default rules conflicted with
  Prettier-formatted tables (`MD013`) and files intentionally starting with an
  import or directive rather than a top-level heading (`MD041`, such as
  `CLAUDE.md` importing `@AGENTS.md`).

## Consequences

Avoids maintaining a suppression config just to keep a Markdown linter from
fighting Prettier. Revisit if structural Markdown issues that Prettier does not
catch become a recurring problem.
