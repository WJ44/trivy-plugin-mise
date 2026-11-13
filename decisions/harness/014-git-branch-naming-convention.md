# 014: Git branch naming convention

- **Status:** Active
- **Finality:** Standard
- **Scope:** `agent-context/harness/commits.md`
- **Date:** 2026-11-08

## Context

Without a consistent branch naming convention, tracking branch purpose and
correlating branches with issues and pull requests becomes inconsistent across
humans and autonomous agents.

## Decision

Prefix branch names with the allowed Conventional Commit types (`<type>/<name>`,
or `<type>/<issue-id>-<slug>` when working on an issue) using lowercase
kebab-case.

## Options Considered

- **Conventional Commit type prefixes (`<type>/<name>`) (chosen):** Reuses the
  repository's commit and PR title vocabulary without maintaining a separate set
  of branch prefixes.
- **Unprefixed issue branches or author-prefixed branches (`user/feature`):**
  Rejected because author prefixes are redundant with git commit attribution and
  hide the type of work at a glance.

## Consequences

Humans and agents use a single predictable naming scheme across commits, PR
titles, and branch names.
