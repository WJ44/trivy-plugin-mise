# 001: Prettier as baseline formatter

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.prettierrc.json`, `.prettierignore`, `lefthook.yml`, `mise.toml`
- **Date:** 2026-11-03

## Context

Most repositories contain Markdown, JSON, YAML, and web files that benefit from
a single consistent formatter out of the box.

## Decision

Use Prettier as the baseline formatter across the repository, delegating
languages with dedicated formatters to their own tools via `.prettierignore`.

## Options Considered

- **Prettier (chosen):** Broad multi-language coverage for Markdown, JSON, YAML,
  and web files with `--ignore-unknown` filtering. No alternative multi-format
  baseline formatter was evaluated.

## Consequences

Requires a JavaScript runtime (see [002](002-node-runtime.md)). File types
handled by a dedicated formatter get their own job in `lefthook.yml` and an
exclusion in `.prettierignore` if Prettier would otherwise touch them.
