# 024: Decision records in decisions/ with finality and options considered

- **Status:** Active
- **Finality:** Standard
- **Scope:** `decisions/`, `decisions/harness/`,
  `agent-context/harness/decisions.md`, `.github/ISSUE_TEMPLATE/decision.md`
- **Date:** 2026-11-13

## Context

When an agent or contributor encounters a convention, tool choice, or constraint
in the repository, the codebase shows _what_ was chosen, but not:

1. **Why** it was chosen and what constraints drove it.
2. **Which options were actually evaluated** (versus never considered).
3. **How final the decision is** — whether it was a quick pragmatic choice open
   to alternatives, a settled default, or a locked invariant.

In addition, decisions span tooling, git conventions, and human-agent workflows
rather than only software architecture, and projects initialized from the
harness need their own top-level `decisions/` numbering without colliding with
inherited harness baseline decisions.

## Decision

1. Record significant design, tooling, and workflow choices as numbered
   **Decision Records** in `decisions/`:
   - Project-specific decisions live directly in `decisions/NNN-<slug>.md` and
     are indexed in `decisions/README.md`.
   - Inherited baseline harness decisions live in
     `decisions/harness/NNN-<slug>.md` and are indexed in
     `decisions/harness/README.md`.
2. Standardize each record around four metadata fields (`Status`, `Finality`,
   `Scope`, `Date`) and four sections (`Context`, `Decision`,
   `Options Considered`, `Consequences`):
   - Separate lifecycle `Status` (`Active` | `Superseded by NNN` | `Deprecated`)
     from commitment `Finality` (`Provisional` | `Standard` | `Locked`).
   - Require `Options Considered` to list only the options _actually evaluated_
     when making the decision (explicitly stating when no alternatives were
     compared).

## Options Considered

- **Decision Records in `decisions/` (with `decisions/harness/` for inherited
  baseline decisions) + separate `Status` and `Finality` (chosen):** Fits both
  architectural and tooling/workflow decisions, keeps project decision numbering
  clean, and gives agents explicit rules on how to treat `Provisional`,
  `Standard`, and `Locked` records.
- **Flat `decisions/` directory mixing harness and project records:** Rejected
  because 20+ harness baseline decisions would clutter a downstream project's
  index and prevent project decisions from starting at `001`.
- **Single combined `Status` field (mixing lifecycle and finality):** Rejected
  because keeping `Status` (`Active` vs. `Superseded`) separate from `Finality`
  (`Provisional` | `Standard` | `Locked`) keeps both dimensions orthogonal and
  scannable.

## Consequences

Every decision record communicates its scope, how open it is to reconsideration
(`Finality`), and which alternatives were actually evaluated
(`Options Considered`), while downstream projects start with a clean top-level
`decisions/` index alongside the inherited baseline in `decisions/harness/`.
