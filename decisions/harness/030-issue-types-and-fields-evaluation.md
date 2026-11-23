# 030: GitHub issue types and fields evaluation

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/labels.json`, `.github/ISSUE_TEMPLATE/`,
  `bin/sync-labels`, `agent-context/harness/issues.md`
- **Date:** 2026-11-18

## Context

GitHub supports repository labels, account/organization-level **issue types**
(defaulting to `Task`, `Bug`, and `Feature`, with up to 25 custom types), and
structured **issue fields** (text, number, date, and select fields). Evaluated
whether adopting issue types—and optionally issue fields—alongside or instead of
the 10-label taxonomy in [016](016-issue-label-taxonomy-and-sync.md) would
improve human-agent issue triage and downstream portability.

## Decision

Continue using repository labels alone via `.github/labels.json` and
`bin/sync-labels` ([016](016-issue-label-taxonomy-and-sync.md)). Do not adopt
GitHub issue types or issue fields at this time.

## Options Considered

- **Labels only (chosen):** Keeps all work-type (`bug`, `enhancement`,
  `decision`, `chore`, `documentation`, `security`) and workflow-status
  (`proposal`, `approved`, `needs-refinement`, `blocked`) metadata declarative,
  versioned in git, and synced per repository via `bin/sync-labels` using only
  repository-scoped `Issues` permissions. Supports multi-label classification
  (e.g. `documentation` alongside `decision`, or `security` alongside `bug`) and
  works uniformly across personal and organization accounts via both the web UI
  and `gh` CLI.
- **Labels + issue types (built-in or custom):** Rejected. Issue type
  definitions live at the user-profile or organization level rather than per
  repository, so downstream repositories cannot declaratively define or sync
  custom types in `.github/` without mutating account-wide settings across
  unrelated repositories (and repository-scoped GitHub App installation tokens
  cannot read or manage user-level type definitions). Issue types are also
  strictly single-valued, preventing multi-type classification. Using only
  GitHub's three built-in defaults (`Task`, `Bug`, `Feature`) would either
  duplicate existing labels—creating drift between web templates and
  non-interactive agent creation—or split the six work types across two
  incompatible query surfaces while still assuming downstream account owners
  have not renamed or disabled those defaults.
- **Labels + issue types + issue fields:** Rejected. Issue fields share the same
  account/organization-global configuration boundary and bot-permission gap, add
  project-management metadata (`Priority`, `Effort`, dates) the harness workflow
  does not use, and—despite REST (`issue_field_values`) and GraphQL API
  support—lack first-class `gh issue` flags or `--json` fields, forcing raw
  `gh api` (REST or GraphQL) calls for routine agent state transitions.

## Consequences

- Issue classification and workflow state remain self-contained in
  `.github/labels.json` and `.github/ISSUE_TEMPLATE/`, synced per repository
  without extra GitHub App permissions or external account setup.
- Work-type and workflow-status labels share one flat label list rather than
  enforcing mutual exclusivity between work categories at the platform level.
- Revisit if GitHub introduces repository-scoped declarative issue types/fields
  with full `gh` CLI support, or if downstream organization projects standardize
  on type-based rollups.
