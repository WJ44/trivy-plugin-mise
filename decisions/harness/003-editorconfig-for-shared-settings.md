# 003: Shared settings in .editorconfig

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.editorconfig`
- **Date:** 2026-11-03

## Context

Line endings, final newlines, indentation, and line width apply to every
formatter and editor in a repository, not just a single tool.

## Decision

Put formatting settings that multiple tools and editors understand in
`.editorconfig`, and specify them only where they differ from the tools'
defaults.

## Options Considered

- **Shared base in `.editorconfig` (chosen):** Avoids duplicating indentation
  and line-ending rules across formatters (`prettier`, `shfmt`, `tombi`),
  editors, and future language tools.
- **Per-tool configuration files only:** Rejected because editors and multiple
  formatters would not inherit shared whitespace defaults.

## Consequences

Formatters and editors automatically share the same base settings, while
tool-specific configuration files only hold options unique to that tool. Most
file types keep their tool's defaults.
