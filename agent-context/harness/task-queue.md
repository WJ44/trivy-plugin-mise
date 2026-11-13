# Task queue orchestration

When asked to tackle multiple issues, process a list of tasks, or work through
an open-ended backlog, an agent must **never** implement multiple issues
sequentially inside a single shared context window. Accumulated context from
earlier tasks degrades reasoning quality, wastes tokens, and risks
cross-contaminating changes across branches.

Instead, use the **dispatcher + fresh-context subagent** workflow:

## 1. Dispatcher and fresh-context subagents

- **Main session as dispatcher:** The primary agent session acts only as a
  lightweight orchestrator. It resolves the task queue, ensures a clean git
  state before each task, spawns one subagent per task, and tracks overall
  progress.
- **Fresh-context implementation subagent per task:** For each task in the
  queue, the dispatcher spawns a general-purpose implementation subagent with a
  clean context (`self` via `invoke_subagent` in Antigravity; general-purpose
  `Agent` in Claude Code).
- **Minimal hand-off prompt:** Pass **only** the target issue or PR number and
  instructions to read and follow the repository conventions (`AGENTS.md`,
  `agent-context/harness/issues.md`, `agent-context/harness/commits.md`, and
  `agent-context/harness/pull-requests.md`). Do not pass implementation context
  or diffs from prior tasks in the queue.

## 2. Input modes

The dispatcher supports two ways of defining the task queue:

1. **Explicit list mode:** The human specifies an ordered list of issue numbers
   and/or PR numbers to work through sequentially (e.g. "Tackle issues 52, 55,
   and 60"). The dispatcher processes only the specified items in the given
   order.
2. **Open-ended queue mode (default allowed task pool):** When asked to work
   through available tasks without an explicit list, the dispatcher queries
   GitHub for eligible work using the following baseline task pool
   (`Provisional` — subject to future tuning):
   - **Open PRs with human-approved review action items:** Open pull requests
     that have pending action items from a review (by a human or an agent review
     subagent) where a human has reviewed the findings and explicitly approved
     acting on them or provided direction (satisfying the
     [Human intervention gate](pull-requests.md#4-human-intervention-gate), e.g.
     via a human comment/review direction or the `approved` label). PRs awaiting
     initial human review after an agent review comment remain gated and must
     not be picked up automatically.
   - **Failing or unsynced Renovate dependency PRs:** Open PRs authored by
     Renovate (`app/renovate` or `renovate[bot]`) whose CI checks are failing or
     whose manifest changed without a matching lockfile update per
     [Handling Renovate dependency PRs](pull-requests.md#handling-renovate-dependency-prs).
     Passing Renovate PRs with synced lockfiles are left for human merge without
     agent intervention.
   - **Ready GitHub issues:** Open issues (`gh issue list --state open`) that
     meet **all** of the following criteria:
     - **Not blocked or awaiting refinement:** Exclude issues carrying the
       `needs-refinement` or `blocked` label.
     - **Human-created or human-approved:** Exclude autonomous agent proposals
       carrying the `proposal` label **unless** they also carry the `approved`
       label and have `- [x] Approved to proceed` checked in the issue body (see
       [Human approval gate](issues.md#human-approval-gate)).
     - **Not reserved for humans:** Exclude issues explicitly marked for human
       action (e.g. a `For human:` title prefix).
     - **No existing open PR:** Exclude any issue that already has an open pull
       request referencing or closing it (`gh pr list --state open`).

## 3. Git isolation between tasks

Tasks run sequentially against the workspace repository and must be strictly
isolated at the git level:

1. **Reset before every task:** Before spawning each task subagent, verify that
   the working tree and index are completely clean (`git status --porcelain` is
   empty) and reset to the latest default branch:

   ```bash
   git checkout main && git pull --ff-only
   ```

2. **Dedicated branch per task:**
   - **For issues:** The task subagent creates its own branch from the updated
     `main` branch (`<type>/<issue-id>-<slug>` per
     [Commits and branches](commits.md#branch-naming)), implements and verifies
     the change, commits, pushes, and opens a pull request following
     [Pull requests](pull-requests.md).
   - **For existing PRs (human-approved review follow-ups or Renovate PRs
     needing intervention):** The task subagent checks out the PR branch
     (`gh pr checkout <pr-number>`), addresses the human-approved review action
     items, CI failure, or unsynced lockfile, verifies formatting/linting,
     commits, and pushes the updates to the PR branch.
3. **Post-PR review hand-off:** After opening or updating a PR, the task
   subagent triggers the required clean-context PR review subagent per
   [Agent PR review workflow](pull-requests.md#agent-pr-review-workflow). Once
   the review comment is posted on that PR, work on that PR stops (respecting
   the [human intervention gate](pull-requests.md#4-human-intervention-gate) for
   that PR) and control returns to the dispatcher to reset to `main` and proceed
   to the next independent task in the queue.

## 4. Failure and blocker recovery

If a task subagent encounters an unresolvable failure, ambiguity, or external
blocker mid-task:

- **No partial state on `main` or incomplete PRs:** Never leave uncommitted or
  half-finished changes in the working tree, never commit broken state to
  `main`, and do not open an incomplete PR that fails acceptance criteria.
- **Clean up local state:** Discard uncommitted changes and untracked files, and
  return the repository to a clean `main` branch
  (`git checkout main && git pull --ff-only`). Delete any unpushed local working
  branch if no work is being preserved.
- **Record the blocker:** Post a concise comment on the issue explaining the
  blocker or ambiguity, and add `needs-refinement` or `blocked` if appropriate.
- **Continue the queue:** Report the skipped or blocked status back to the
  dispatcher so it can proceed to the next task in the queue.

## 5. Completion summary

Once every item in the explicit list or eligible open-ended queue has been
processed, the dispatcher stops and reports a concise summary to the human
covering:

- Pull requests opened or updated (with PR numbers and links).
- Tasks skipped or blocked (with issue/PR numbers and the reason).
