# Issue refinement

Issues labeled `needs-refinement` or `proposal` often start as rough ideas or
autonomous agent observations that require design decisions, scope boundaries,
and concrete acceptance criteria before an agent can implement them
autonomously. Keeping this interactive workflow in its own reference file avoids
loading refinement instructions when opening or implementing ready issues.

## 1. Strict role separation

- **Human decides:** The human makes all design, scope, trade-off, and priority
  decisions.
- **Agent researches, suggests, and asks:** The agent inspects the issue
  (`gh issue view <number> --json title,body,labels,comments,author`) and
  relevant repository files and decision records
  ([`decisions/`](../../decisions/) and
  [`decisions/harness/`](../../decisions/harness/)) in **read-only** mode. It
  surfaces architectural interactions, existing conventions, or edge cases,
  proposes concrete options with a recommended choice first, and asks focused
  questions.
- **No code edits or silent assumptions:** During refinement, the agent must
  **never** modify repository files, create implementation branches, or make
  silent assumptions about ambiguous requirements.

## 2. Entry modes (single issue or batch)

- **Single-issue mode:** Refine a specific issue requested by the human (e.g.
  `refine issue <number>`).
- **Batch / queue mode:** Step through open issues awaiting refinement or review
  one by one:
  - Issues needing refinement:
    `gh issue list --state open --label needs-refinement`
  - Unapproved agent proposals: `gh issue list --state open --label proposal`
    (excluding issues that already carry `approved`)
  - Complete the full refinement, sign-off, and GitHub update cycle for one
    issue before moving to the next.

## 3. Context gathering and structured question rounds

- **Read-only codebase, decision, and PR/issue context:** Read the issue body
  and comments, then inspect the affected files and active decision records
  (delegating broad codebase exploration to a read-only subagent per
  `AGENTS.md`) alongside recent related issues and pull requests (e.g.
  `gh pr list --state all --limit 10` and
  `gh issue list --state all --limit 10`, inspecting relevant PR review comments
  or issue discussions via `--json`). Surfacing recently merged or in-flight
  work early catches conventions and human steering that have not yet been
  captured in a decision record. Distinguish between questions already answered
  by existing codebase conventions, recent precedent, or `Standard`/`Locked`
  decisions and questions that genuinely require human judgment.
- **Structured multiple-choice questions:** Use the host tool's structured
  question UI (`ask_question` in Antigravity, `AskUserQuestion` in Claude Code)
  for multi-choice design, scope, and trade-off questions (or to confirm closing
  a resolved issue).
- **Recommended option first:** Put the recommended choice first and prefix it
  with `(Recommended)` (e.g. `(Recommended) <option and brief rationale>`).
- **Concise question rounds:** Keep each round to 2–4 high-leverage questions
  rather than an exhaustive questionnaire.
- **Iterative follow-ups:** When a human's answer opens a follow-up trade-off or
  edge case, conduct another brief round until design, scope, and acceptance
  criteria converge.
- **Fast-path for resolved or no-change issues:** If read-only research directly
  answers an investigative issue (e.g. _"Check if it makes sense to..."_) or
  shows the issue is already solved, obsolete, or requires no repository
  changes, present the findings in the conversation and offer closing the issue
  as `not planned` (with a brief summary comment) directly via `ask_question` /
  `AskUserQuestion`—putting `(Recommended)` on closing as `not planned`
  alongside options to keep the issue open or defer. Skip drafting a full
  replacement issue body (Section 4) unless the human chooses to keep the issue
  open and scope repository changes.

## 4. Drafting and human sign-off

When the issue will remain open for implementation (skip this section if the
human agrees via the Section 3 fast-path to close the issue as `not planned` or
defer):

- **Present a complete draft before editing GitHub:** Once decisions converge,
  present the full proposed issue body in the conversation for human sign-off
  before running `gh issue edit`.
- **Follow issue conventions and templates:** Structure the draft
  (`Problem / Motivation`, `Proposed Solution`, `Scope & Boundaries`, and
  testable `Acceptance Criteria`, or the matching template in
  [`.github/ISSUE_TEMPLATE/`](../../.github/ISSUE_TEMPLATE/)) following
  [Issues](issues.md)
  ([Writing acceptance criteria](issues.md#writing-acceptance-criteria) and
  [Bounding scope](issues.md#bounding-scope)).
- **Check agent docs, human docs, and decision coverage:** When drafting
  `Scope & Boundaries` and `Acceptance Criteria`, evaluate whether the change
  requires updates across `agent-context/` (agent instructions, including
  `agent-context/harness/`), `docs/` (human documentation, including
  `docs/harness/`), and `decisions/` (decision records per
  [Decisions](decisions.md#when-to-record-a-decision), including
  `decisions/harness/`). Include required layers explicitly in target files and
  acceptance criteria, or mark unaffected layers out of scope.
- **Explicit sign-off:** Ask the human to confirm the draft and whether to apply
  the update and transition labels.

## 5. Updating the issue and labels

After the human confirms the draft or closure decision, update the issue on
GitHub:

- **Approved for implementation:** Update the body, remove `needs-refinement`
  (if present), and add `approved`:

  ```bash
  gh issue edit <number> --body-file - --remove-label needs-refinement --add-label approved
  ```

  For `proposal` issues, also check `- [x] Approved to proceed` in the issue
  body per the [Human approval gate](issues.md#human-approval-gate).

- **Deferred or still blocked:** Keep `needs-refinement` or apply `blocked` as
  directed by the human.

- **Declined:** Close the issue as not planned
  (`gh issue close <number> --reason "not planned"`). When closing due to
  findings uncovered during refinement (for example, an investigative question
  answered by research or an already-solved/obsolete issue), post a brief
  summary comment so the rationale is preserved on GitHub:

  ```bash
  gh issue close <number> --reason "not planned" --comment "<findings>"
  ```
