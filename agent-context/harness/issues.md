# Issues

Issues in this repository originate from two sources:

1. **Humans** filing bug reports, defining features/tasks, or framing design
   decisions.
2. **Autonomous agents** discovering bugs, tech debt, or improvement
   opportunities outside the scope of their current task.

## Issues as the human-agent communication layer

Issues serve as the primary contract and communication interface between humans
and agents:

- Most human-created issues are intended for agents to pick up and implement
  autonomously.
- Agent-created issues capture autonomous discoveries for humans to review,
  shape, and approve.

Because the implementer is usually an autonomous agent, **acceptance criteria
are critical**:

- **Agent loop stopping condition:** Autonomous agents iterate against the
  acceptance criteria to verify the task is complete before opening a PR.

- **Human steering:** On human-created issues, acceptance criteria define the
  exact boundaries of satisfaction. On agent-created proposals, reviewing and
  refining the acceptance criteria is the primary mechanism (alongside approval)
  for a human to steer the implementation.

- When an issue is picked up or inspected via `gh`, prefer structured JSON
  output over plain `gh issue view <id> --comments`, which line-wraps Markdown
  prose and can truncate long bodies or comments:

  ```bash
  gh issue view <id> --json title,body,labels,comments,author
  ```

  When updating an issue body from the CLI, pass Markdown via
  `gh issue edit <id> --body-file -` to preserve formatting and checklists.

### Writing acceptance criteria

- Criteria must be concrete, objective, and testable (e.g. specific commands and
  exit codes, observable behavior, tests added, or files updated).
- Keep criteria focused on **task-specific** conditions of satisfaction.
  Repo-wide quality gates (`lefthook run format-check`,
  `lefthook run lint-check`, `commitlint`) apply to every change automatically
  via hooks and CI, so they do not need to be repeated on every issue.
- Avoid vague goals like "improve performance" or "clean up script" without
  verifiable conditions.

### Bounding scope

To prevent autonomous agents from drifting into unrelated refactoring:

- Specify target files, directories, or components to touch.
- Call out explicit non-goals or files that must not be modified.

### Declaring dependencies

When an issue depends on one or more prerequisite issues in the same repository:

- Declare each prerequisite explicitly in the issue body using
  `Depends on #<id>` or `Blocked by #<id>` (for example, `Depends on #42` or
  `Blocked by #42`).
- In the [task queue](task-queue.md), carrying the `blocked` label does **not**
  exclude an issue from open-ended queue execution if **all** of its declared
  blockers are prerequisite `#<id>` issues that are either closed/merged, ready
  in the current queue, or backed by a queued or qualifying open PR (allowing
  the dispatcher to order and stack linear chains automatically).
- Keep the `blocked` label alongside an explicit prose description whenever an
  issue is gated by an external dependency, pending decision, unready
  prerequisite, or anything other than qualifying `#<id>` issues so the
  dispatcher keeps it excluded until resolved.

## Templates

Issue templates live in
[`.github/ISSUE_TEMPLATE/`](../../.github/ISSUE_TEMPLATE/) and use Markdown
(`.md`) so they work identically in the GitHub web UI and via
`gh issue create --template`:

| Template            | Filename             | Purpose                                                               | Default Labels |
| ------------------- | -------------------- | --------------------------------------------------------------------- | -------------- |
| **Agent Proposal**  | `agent_proposal.md`  | Autonomous observations, tech debt, or improvements noticed by agents | `proposal`     |
| **Bug Report**      | `bug_report.md`      | Reproducible defects, broken behavior, or regressions                 | `bug`          |
| **Feature / Task**  | `feature_request.md` | New capabilities, tool integrations, or improvements                  | `enhancement`  |
| **Design Decision** | `decision.md`        | Design choices, trade-off evaluations, and decision record candidates | `decision`     |

Free-form issues are also enabled (`blank_issues_enabled: true` in
[`config.yml`](../../.github/ISSUE_TEMPLATE/config.yml)).

## Labels

Issue labels are defined declaratively in
[`.github/labels.json`](../../.github/labels.json) and synchronized via
[`bin/sync-labels`](../../bin/sync-labels). The taxonomy stays flat and compact
(10 labels) for fast visual scanning and frictionless CLI filtering
(`gh issue list --label <name>`).

### Work Types

- `bug` (`#e11d48`): Unexpected defect, broken behavior, or regression.
- `enhancement` (`#0284c7`): New feature, capability, or improvement.
- `decision` (`#6366f1`): Design choice, trade-off evaluation, or decision
  record.
- `chore` (`#64748b`): Maintenance, dependencies, tooling, or configuration
  upkeep.
- `documentation` (`#0ea5e9`): Documentation or agent-context updates.
- `security` (`#b91c1c`): Security vulnerability, credential handling, or
  sandbox boundary.

### Workflow & Status

- `proposal` (`#8b5cf6`): Autonomous agent proposal awaiting human review.
- `approved` (`#10b981`): Approved by a human and ready for agent
  implementation.
- `needs-refinement` (`#eab308`): Needs human input, design clarification, or
  acceptance criteria refinement before an agent picks it up (see
  [Issue refinement](issue-refinement.md)).
- `blocked` (`#f59e0b`): Blocked by an external dependency, prerequisite issue,
  or pending decision (in the [task queue](task-queue.md), excluded unless all
  declared blockers are qualifying `#<id>` prerequisites per
  [Declaring dependencies](#declaring-dependencies)).

Declined or duplicate issues are closed using GitHub's native **Close as not
planned** state reason rather than extra resolution labels.

### Synchronizing labels

To synchronize labels with GitHub:

```bash
bin/sync-labels [--prune] [--dry-run]
```

## Autonomous agent proposals

When an agent notices a bug, limitation, code smell, or improvement opportunity
that is outside the scope of its current task:

1. **Do not expand task scope:** Do not silently bundle unrelated fixes or
   refactoring into the current branch.
2. **Open a proposal issue:** Create an issue structured after
   [`agent_proposal.md`](../../.github/ISSUE_TEMPLATE/agent_proposal.md) via
   `gh issue create`, applying the `proposal` label plus the relevant work-type
   label (e.g. `bug`, `enhancement`, `chore`, `security`, `decision`).
3. **Provide self-contained context:** The human reviewer was not in the agent's
   session. Always include:
   - **Context & Origin:** What task, branch, PR, or file the agent was working
     on when this was noticed.
   - **Problem / Observation:** Concrete description with file/line references
     or reproduction steps.
   - **Proposed Approach & Alternatives:** Recommended solution, trade-offs, and
     alternatives so the human can steer the choice.
   - **Scope & Boundaries:** Target files/areas and explicit non-goals.
   - **Proposed Acceptance Criteria:** Draft testable criteria for the human to
     review and refine.

## Human approval gate

Autonomous agent proposals **must not be picked up and implemented
immediately**. They wait for human review and approval:

- Agent proposals carry the `proposal` label so humans can triage them quickly
  (e.g. during a coffee break via `gh issue list --label proposal` or
  `is:issue is:open label:proposal`).
- To approve a proposal, a human reviews and refines the **Proposed Acceptance
  Criteria**, checks `- [x] Approved to proceed` in the issue body (1-tap on web
  or mobile), and adds the `approved` label (making approved proposals queryable
  via `gh issue list --label approved`).
- When asked to interactively refine a `needs-refinement` or `proposal` issue
  with a human, follow [Issue refinement](issue-refinement.md).
- To decline a proposal, the human simply closes the issue as **not planned**.
- An agent may only implement a proposal once it is approved and its acceptance
  criteria are finalized.
- When starting work on an issue, create a dedicated branch following
  [Commits and branches](commits.md#branch-naming) (e.g.
  `feat/<issue-id>-<slug>` or `fix/<issue-id>-<slug>`).
