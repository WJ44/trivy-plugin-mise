# 026: Separate harness and project docs in agent-context/

- **Status:** Active
- **Finality:** Standard
- **Scope:** `agent-context/`, `agent-context/harness/`, `AGENTS.md`
- **Date:** 2026-11-13

## Context

`AGENTS.md` is kept small by linking out to on-demand reference files in
`agent-context/`. Downstream projects initialized from the harness inherit the
harness's baseline operational docs (git conventions, PR/issue workflows,
linting, scanning) while also adding project-specific reference docs. Mixing
both in a flat `agent-context/` directory obscures ownership and makes future
harness updates harder to apply without touching project docs.

## Decision

Separate `agent-context/` into two levels:

- **Project-specific reference docs** live directly under `agent-context/`.
- **Shared harness reference docs** live under `agent-context/harness/`,
  matching the `decisions/` vs. `decisions/harness/` split (see
  [024](024-decision-records-format-and-workflow.md)).

## Options Considered

- **Separate `agent-context/harness/` subdirectory (chosen):** Clearly
  distinguishes harness-managed reference docs from project-specific docs and
  allows downstream projects to update harness docs without colliding with
  project files.
- **Flat `agent-context/` directory:** Rejected because inherited harness files
  and project files share the same namespace, complicating ownership and
  upstream harness syncs.

## Consequences

Downstream projects keep their own domain and architecture context directly in
`agent-context/` while `agent-context/harness/` remains cleanly owned by the
harness baseline.
