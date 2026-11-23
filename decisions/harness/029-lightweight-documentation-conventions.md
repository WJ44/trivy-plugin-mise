# 029: Lightweight documentation in docs/ and high-level decision records

- **Status:** Active
- **Finality:** Standard
- **Scope:** `docs/`, `docs/harness/`, `decisions/`, `decisions/harness/`,
  `agent-context/harness/decisions.md`, `AGENTS.template.md`,
  `README.template.md`, `bin/harness.py`
- **Date:** 2026-11-18

## Context

Apart from `decisions/` (rationale and trade-offs) and `agent-context/`
(operational rules for agents), the repository had no designated home for
concise, human-focused usage and conceptual documentation. Without a separate
documentation layer, agents placed functional specifications, CLI flags, and
library/parser internals into Decision Records so context would not be lost.

## Decision

Split technical and workflow documentation across four distinct layers:

1. **Code, docstrings, and CLI `--help`:** Primary source of truth for
   implementation details, internal mechanics, and exhaustive flag syntax.
2. **`docs/` and `docs/harness/`:** Concise, human-focused usage guides,
   workflows, and conceptual overviews—project-specific directly in `docs/` and
   inherited harness baseline docs in `docs/harness/` (matching `decisions/` and
   `agent-context/`). Indexed from `README.md`, synced downstream via
   `bin/harness.py`, and excluded from tracked history in release mode.
3. **`agent-context/` and `agent-context/harness/`:** Imperative operational
   rules, conventions, and workflows for agents, indexed in `AGENTS.md`.
4. **`decisions/` and `decisions/harness/`:** High-level problem context, chosen
   approach, evaluated options with trade-offs, resulting invariants, and
   finality—free of low-level implementation mechanics, code snippets, and usage
   guides.

## Options Considered

- **`docs/` (with `docs/harness/` for baseline docs) + four-way responsibility
  split (chosen):** Mirrors the `decisions/` and `agent-context/` namespace
  split, keeps `decisions/` and `AGENTS.md` lean, and integrates cleanly with
  downstream sync and release-mode exclusion.
- **Expand `README.md` and `agent-context/` without a `docs/` directory:**
  Rejected because `README.md` should stay a concise repository entrypoint and
  `agent-context/` is framed around imperative agent rules rather than human
  usage guides.
- **Index every `docs/` file individually in `AGENTS.template.md`:** Rejected to
  keep `AGENTS.md` small; a brief note pointing agents to `docs/` and
  `docs/harness/` (indexed in `README.md`) lets agents read usage docs on demand
  when learning how a tool or feature works.

## Consequences

- Decision records remain concise and rationale-focused; implementation details
  live in code and `--help`, and human usage guides live in `docs/` or
  `docs/harness/`.
- `bin/harness.py` syncs `docs/harness/` across downstream projects and keeps it
  gitignored locally in release mode without scaffolding an empty `docs/` folder
  until a project adds documentation.
