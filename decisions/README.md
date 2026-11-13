# Decision Records

Numbered records of design, tooling, workflow, and architectural decisions in
this repository.

- **Project decisions** live directly in `decisions/` (`001-<slug>.md`,
  `002-<slug>.md`, ...) and are indexed below.
- **Baseline harness decisions** inherited across projects live in
  [`decisions/harness/`](harness/README.md).

For the full template, metadata definitions (`Status` vs. `Finality`), and
instructions on when and how to record or supersede a decision, see
[`agent-context/harness/decisions.md`](../agent-context/harness/decisions.md).

## Finality Levels at a Glance

- **`Provisional`** — Pragmatic "go with this for now" choice made with minimal
  evaluation; explicitly open to reconsideration.
- **`Standard`** — Evaluated trade-offs; settled default unless new information,
  requirements, or unconsidered options emerge.
- **`Locked`** — Foundational constraint; do not revisit without explicit human
  direction.

## Project Decisions

| ID                                      | Title                                          | Status | Finality | Date       |
| --------------------------------------- | ---------------------------------------------- | ------ | -------- | ---------- |
| [001](001-language-and-distribution.md) | Implementation language and distribution model | Active | Standard | 2026-11-13 |
