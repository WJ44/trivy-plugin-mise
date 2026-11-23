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

## Keeping decision records high-level

Decision records capture **rationale and commitment**, not how a tool is
implemented or operated. Keep every record concise and strictly scoped:

- **Belongs in `decisions/`:** Problem context and drivers, the chosen approach
  stated concisely, options actually evaluated with high-level trade-offs,
  resulting invariants or consequences, and `Finality`.
- **Does not belong in `decisions/`:** Low-level implementation mechanics,
  parser or library internals (such as AST node tags or internal function
  names), configuration or code snippets, exhaustive CLI flag tables, and usage
  guides.

Put operational and implementation details in their proper home instead:

- **Implementation mechanics & CLI flag syntax:** Code comments, docstrings, and
  CLI `--help` output.
- **Human-focused usage guides, workflows, & conceptual overviews:**
  [`docs/`](../../docs/) (project-specific) or
  [`docs/harness/`](../../docs/harness/) (baseline harness tools), linked from
  [`README.md`](../../README.md).
- **Agent operational rules & workflows:** [`agent-context/`](..)
  (project-specific) or [`agent-context/harness/`](.) (baseline harness rules),
  linked from [`AGENTS.md`](../../AGENTS.md).

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

When a single Decision Record covers sub-decisions with different commitment
levels, state each level explicitly in the individual record's `- **Finality:**`
header (for example,
`Standard (dispatcher pattern; default open-ended task filter is Provisional)`).
In the index tables ([`decisions/README.md`](../../decisions/README.md) and
[`decisions/harness/README.md`](../../decisions/harness/README.md)), keep the
`Finality` cell to a single compact token (`Provisional`, `Standard`, or
`Locked`) representing the record's primary commitment level so adding or
updating rows does not widen the column and trigger full-table Prettier
re-padding.

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
2. Pick the next sequential number `NNN` in the target directory. Check both the
   existing files in `<target>/` and open PRs
   (`gh pr list --state open --limit 100 --json number,files`) so concurrent PRs
   do not claim the same `NNN` up front.
3. Create `<target>/NNN-<slug>.md` (or `<target>/NNN-<slug>-evaluation.md` for
   standalone evaluation records) using the template above. Keep it concise and
   high-signal.
4. Add an unpadded entry (`| [NNN](...) | ... |`) to the
   `<!-- prettier-ignore -->` index table in `<target>/README.md` using a
   single-token `Finality` value (`Provisional`, `Standard`, or `Locked`). Keep
   `<!-- prettier-ignore -->` above the table and avoid padding cells to column
   widths so editing or appending one row touches only that single line.
5. Update any companion docs needed—without duplicating details in the decision
   record:
   - Ongoing operational rules for agents go in [`agent-context/`](..) (or
     [`agent-context/harness/`](.)).
   - Human-focused usage guides or conceptual overviews go in
     [`docs/`](../../docs/) (or [`docs/harness/`](../../docs/harness/)) and are
     linked from [`README.md`](../../README.md).

### Updating, evaluating, or superseding a decision

Choose among three patterns based on whether the existing decision's core choice
stands and whether the change closes a dedicated evaluation issue:

1. **In-place refinement or extension of an active decision:** Use when
   clarifying, extending, or adjusting the scope of an active decision without
   reversing or replacing its core choice—or when adding a newly evaluated
   option surfaced while revisiting that record (for example, extending `ruff`
   to formatting in [`009`](../../decisions/harness/009-ruff-for-python.md),
   adding interactive refinement in
   [`013`](../../decisions/harness/013-issue-templates-and-autonomous-proposals.md),
   broadening CI triggers in
   [`015`](../../decisions/harness/015-ci-pipeline.md), adding bounded auto-fix
   in [`019`](../../decisions/harness/019-agent-pr-review-process.md), or adding
   stacked PR orchestration in
   [`022`](../../decisions/harness/022-multi-issue-orchestration.md)). Update
   the existing record in place (`Context`, `Decision`, `Options Considered`,
   `Scope`, `Consequences`, `Finality` if appropriate, and `Date`), and sync its
   row in the corresponding index
   ([`decisions/README.md`](../../decisions/README.md) or
   [`decisions/harness/README.md`](../../decisions/harness/README.md)) while
   keeping the table's `Finality` cell to a single token.
2. **Standalone numbered evaluation record (`NNN-<slug>-evaluation.md`):** Use
   when closing a dedicated evaluation or `decision`-labeled issue whose outcome
   retains the existing baseline tool, workflow, or convention without adopting
   the evaluated alternative (for example,
   [`010-markdownlint-evaluation.md`](../../decisions/harness/010-markdownlint-evaluation.md)
   or
   [`030-issue-types-and-fields-evaluation.md`](../../decisions/harness/030-issue-types-and-fields-evaluation.md)
   retaining
   [`016`](../../decisions/harness/016-issue-label-taxonomy-and-sync.md)).
   Create a new sequential record (`NNN-<slug>-evaluation.md`) in the
   appropriate directory (`decisions/` or `decisions/harness/`) with
   `Status: Active`, reference the retained decision(s) without marking them
   `Superseded`, and add the new row to the corresponding index.
3. **Superseding a reversed or replaced decision (`MMM-<slug>.md`):** Use when
   reversing or replacing the core choice of an earlier decision rather than
   refining it in place. Create a new sequential record (`MMM-<slug>.md`) in the
   appropriate directory (`decisions/` or `decisions/harness/`) referencing the
   old record in `Context`, set the old record's `Status` to
   `Superseded by [MMM](MMM-slug.md)`, and update both rows in the corresponding
   index ([`decisions/README.md`](../../decisions/README.md) or
   [`decisions/harness/README.md`](../../decisions/harness/README.md)).

### Resolving `NNN` collisions during `main`-sync

If two concurrent branches add a decision record in the same directory with the
same sequential number `NNN`:

1. **Detect the collision against `origin/main`:** During a `main`-sync or
   rebase pass (`git fetch origin main && git merge origin/main`), check whether
   `origin/main` introduced a decision record with your `NNN` prefix in
   `decisions/` or `decisions/harness/` (also reported by
   `bin/validate-decisions.py` as `duplicate decision ID 'NNN'`).
2. **Pick the next unused `NNN`:** Check both the merged `origin/main` files in
   the target directory and open PRs
   (`gh pr list --state open --limit 100 --json number,files`) to choose the
   next unused sequential ID `MMM`.
3. **Renumber every reference in your branch:**
   - Rename the record (`git mv <target>/NNN-<slug>.md <target>/MMM-<slug>.md`).
   - Update the `# MMM: <Title>` heading (and any self-references) inside the
     renamed file.
   - If your PR supersedes an older decision, update that record's `Status:`
     bullet (and index row) to `Superseded by [MMM](MMM-<slug>.md)`.
   - Update the row in `<target>/README.md` to `[MMM](MMM-<slug>.md)` in
     numerical order using an unpadded `| ... |` row under
     `<!-- prettier-ignore -->`.
   - Update any references to `NNN` in companion docs, tests, or the PR
     description, then re-run `mise x -- lefthook run format-check` and
     `mise x -- lefthook run lint-check` to confirm `bin/validate-decisions.py`,
     `rumdl`, and Prettier all pass.
