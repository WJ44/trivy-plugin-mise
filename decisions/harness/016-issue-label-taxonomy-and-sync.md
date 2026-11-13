# 016: Issue label taxonomy and synchronization

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/labels.json`, `bin/sync-labels`,
  `agent-context/harness/issues.md`
- **Date:** 2026-11-09

## Context

Default GitHub issue labels include generic tags (`good first issue`,
`help wanted`, `invalid`, `question`) that do not fit human-agent workflows,
while lacking work-type and workflow-state labels needed for triage (`proposal`,
`approved`, `needs-refinement`, `blocked`, `decision`, `chore`, `security`).

## Decision

Define a flat 10-label taxonomy in `.github/labels.json` synced via
`bin/sync-labels`:

- **Work Types (6):** `bug`, `enhancement`, `decision`, `chore`,
  `documentation`, `security`.
- **Workflow & Status (4):** `proposal`, `approved`, `needs-refinement`,
  `blocked`.

Use GitHub's native "Close as not planned" state reason instead of `wontfix` or
`duplicate` labels.

## Options Considered

- **Flat 10-label taxonomy in `.github/labels.json` synced via `bin/sync-labels`
  (chosen):** Fast to scan in the web UI, frictionless to filter in `gh` CLI,
  and declaratively synced in one command.
- **Prefixed/namespaced labels (`type:bug`, `status:approved`):** Rejected
  because with only 10 labels the prefixes add visual clutter and CLI typing
  overhead.
- **Resolution labels (`wontfix`, `duplicate`, `invalid`):** Rejected in favor
  of GitHub's built-in "Close as not planned" reason.

## Consequences

Issues use a compact, non-overlapping label set that drives human-agent triage
and approval, and repositories can synchronize labels declaratively via
`bin/sync-labels`.
