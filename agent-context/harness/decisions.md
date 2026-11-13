# Decisions

Design, tooling, workflow, and architectural decisions are recorded as numbered
Decision Records in [`decisions/`](../../decisions/):

- **Project decisions** live directly in [`decisions/`](../../decisions/)
  (`decisions/NNN-<slug>.md`) and are indexed in
  [`decisions/README.md`](../../decisions/README.md).
- **Baseline harness decisions** inherited across projects live in
  [`decisions/harness/`](../../decisions/harness/) and are indexed in
  [`decisions/harness/README.md`](../../decisions/harness/README.md).

Decisions span more than software architecture — they also cover tool selection,
git conventions, workflows, and project configuration. Calling them **Decision
Records** rather than ADRs avoids overloading "architecture" while keeping a
familiar, lightweight record format.

## Why decision records matter for agents

When an agent encounters a convention, tool choice, or constraint in the repo,
the codebase shows _what_ was chosen, while `decisions/` explains:

1. **Why** it was chosen and what constraints drove it.
2. **Which options were actually evaluated** (and which were not).
3. **How final the decision is** — whether it was a quick "go with this for now"
   choice or a deliberate, locked invariant.

Most decisions are subject to change when new information, requirements, or
better options appear. Knowing both the options considered and the commitment
level prevents two opposite failure modes: treating a hasty default as sacred,
or re-litigating a settled foundational rule.

## Template

Every file in [`decisions/`](../../decisions/) is named `NNN-<slug>.md`
(three-digit zero-padded number, lowercase kebab-case slug) and follows this
structure:

```markdown
# NNN: <Concise title>

- **Status:** Active | Superseded by [MMM](MMM-slug.md) | Deprecated
- **Finality:** Provisional | Standard | Locked
- **Scope:** <Files, tools, workflows, or subsystems governed>
- **Date:** YYYY-MM-DD

## Context

<Problem, drivers, and constraints that prompted the decision.>

## Decision

<The chosen approach, stated concisely.>

## Options Considered

<Exhaustive list of options actually evaluated during the decision process.>

## Consequences

<Trade-offs, resulting invariants, and conditions or triggers for revisiting.>
```

## Metadata semantics

### Status (lifecycle state)

- **`Active`** — Currently in effect.
- **`Superseded by [MMM](MMM-slug.md)`** — Replaced by a newer decision record.
  Keep the file in `decisions/` so the historical reasoning stays intact.
- **`Deprecated`** — Retired without a direct replacement (e.g. the feature or
  tool was removed).

### Finality (commitment level)

`Status` tracks whether a decision is active; `Finality` tracks how thoroughly
it was evaluated and how open it is to reconsideration:

| Finality          | Meaning                                                                                                                             | How agents should treat it                                                                                                                                                 |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`Provisional`** | Pragmatic "go with this for now" choice made quickly or with minimal evaluation. Explicitly open for later reconsideration.         | Follow it for routine work, but actively propose alternatives (via an [Agent Proposal issue](issues.md#autonomous-agent-proposals)) if friction or better options surface. |
| **`Standard`**    | Trade-offs were evaluated and the choice is settled, but remains open to change if new information, requirements, or options arise. | Follow it by default. Propose reconsidering only when concrete new information, unconsidered options, or changed constraints justify revisiting it.                        |
| **`Locked`**      | Foundational principle or deliberate constraint established by the human maintainer.                                                | Treat as a hard invariant. Do not propose replacing or bypassing it unless explicitly asked by a human.                                                                    |

### Options Considered (exhaustive of what was evaluated)

The `Options Considered` section must be an **exhaustive list of the options
actually evaluated** when the decision was made — **not** an exhaustive list of
every option that exists in the ecosystem.

- If three tools were compared, list those three and why each won or lost.
- If a decision was made pragmatically without evaluating alternatives ("picked
  X because it was familiar / already installed"), state that explicitly (e.g.
  _"Only `<chosen option>` was evaluated; adopted pragmatically without
  comparing alternatives."_).
- This makes it immediately clear when a new option surfaces later whether that
  option was previously rejected for a reason or simply never considered.

## Recording or updating a decision

### When to record a decision

Create a decision record when:

- Closing an issue labeled `decision` (using
  [`.github/ISSUE_TEMPLATE/decision.md`](../../.github/ISSUE_TEMPLATE/decision.md)).
- Adding or replacing a tool, linter, formatter, runtime, or dependency policy.
- Establishing or changing a repository convention, security boundary, or
  human-agent workflow.
- Making a deliberate "go with this for now" (`Provisional`) choice that future
  sessions might otherwise mistake for a permanent requirement or an accidental
  oversight.

### Adding a new decision record

1. **Choose the right directory:**
   - **Project decisions** (architecture, product choices, project dependencies,
     or downstream overrides of a harness default) go directly in
     [`decisions/`](../../decisions/) and are indexed in
     [`decisions/README.md`](../../decisions/README.md).
   - **Harness baseline decisions** (shared tooling, git, and agent-workflow
     conventions inherited across projects) live in
     [`decisions/harness/`](../../decisions/harness/) and are indexed in
     [`decisions/harness/README.md`](../../decisions/harness/README.md).
     Downstream projects should not edit `decisions/harness/` directly—record a
     project decision in `decisions/` instead.
2. Pick the next sequential number `NNN` in the target directory.
3. Create `<target>/NNN-<slug>.md` using the template above. Keep it concise and
   high-signal.
4. Add an entry to the index table in `<target>/README.md`.
5. If the decision establishes ongoing operational rules for agents, update the
   relevant reference file in [`agent-context/`](..) as well.

### Superseding or updating an existing decision

- **Minor clarification or new option evaluated without changing the outcome:**
  Update the existing record in place (add the newly evaluated option to
  `Options Considered`, adjust `Finality` if appropriate, and update `Date`).
- **Changing the decision:** Create a new record `MMM-<slug>.md` referencing the
  old one in `Context`, set the old record's `Status` to
  `Superseded by [MMM](MMM-slug.md)`, and update both rows in
  [`decisions/README.md`](../../decisions/README.md).
