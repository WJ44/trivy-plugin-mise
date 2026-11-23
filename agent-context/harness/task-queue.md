# Task queue orchestration

When asked to tackle multiple issues, process a list of tasks, or work through
an open-ended backlog, an agent must **never** implement multiple issues
sequentially inside a single shared context window. Accumulated context from
earlier tasks degrades reasoning quality, wastes tokens, and risks
cross-contaminating changes across branches.

Instead, use the **dispatcher + fresh-context subagent** workflow:

## 1. Dispatcher and fresh-context subagents

- **Main session as dispatcher:** The primary agent session acts only as a
  lightweight orchestrator. It resolves the task queue, orders prerequisite
  tasks ahead of dependents, checks
  [stacking preconditions](#stacking-preconditions), ensures a clean git state
  before each task, spawns one subagent per task, and tracks overall progress.
- **Fresh-context implementation subagent per task:** For each new issue (or
  open PR picked up in a separate/later session), the dispatcher spawns a
  general-purpose implementation subagent with a clean context (`self` via
  `invoke_subagent` in Antigravity; general-purpose `Agent` in Claude Code). For
  review follow-ups while the original authoring subagent is still available in
  the active session, reuse/message that original subagent (`send_message` in
  Antigravity / resuming the task agent in Claude Code) to preserve context and
  save tokens, while still spawning a **separate clean-context subagent** for
  the PR review / re-review itself.
- **Minimal hand-off prompt:** Pass **only** the target issue or PR number and
  instructions to read and follow the repository conventions (`AGENTS.md`,
  `agent-context/harness/issues.md`, `agent-context/harness/commits.md`,
  `agent-context/harness/pull-requests.md`, and
  `.github/PULL_REQUEST_TEMPLATE/agent.md`—never
  `.github/PULL_REQUEST_TEMPLATE.md`). When stacking a dependent issue onto an
  unmerged parent PR, also pass **only** the parent branch name
  (`<parent-branch>`) and parent PR number (`#<parent-pr>`). Do not pass prior
  conversation transcripts, implementation reasoning, or diffs from earlier
  tasks in the queue.
- **Workspace selection (`share`/worktree vs. `inherit`):**
  - **Normal mode (tracked harness files — default `share`/worktree):** When
    harness files (`AGENTS.md`, `.agents/`, `hooks/post-edit`,
    `bin/wait-pr-checks`, `agent-context/harness/`) are tracked in git, launch
    task, sibling-sync, and clean-context PR review subagents in isolated git
    worktrees (`Workspace: "share"` in Antigravity; `isolation: "worktree"` in
    Claude Code). `mise` auto-trusts worktrees of a trusted repo, shared
    `.git/hooks` (`pre-commit`, `commit-msg`) and tracked `.agents/hooks.json` +
    `hooks/post-edit` fire normally, and each subagent gets an isolated checkout
    that cannot collide with the dispatcher. Note that gitignored
    `.claude/settings.local.json` (`autoMemoryDirectory`) and per-path trust
    entries (`~/.claude.json`, Antigravity `trustedWorkspaces`) do not transfer
    automatically to dynamic worktree paths.
  - **Release mode (`inherit` fallback):** When `.gitignore` contains
    `# --- harness:release:start ---` (or `AGENTS.md` / `hooks/post-edit` are
    gitignored and not hydrated into fresh worktrees), fall back to
    `Workspace: "inherit"` so subagents retain hydrated harness instructions,
    hooks, and helper scripts.
- **Workspace exclusivity when using `inherit`:** Whenever a subagent shares the
  dispatcher's workspace (`Workspace: "inherit"`), the dispatcher must wait for
  it to finish and must **not** execute any `git` commands (not even read-only
  `git status` or `git branch`) or read/write workspace files while that
  subagent is active. Never batch `run_command` (or workspace read/write tool
  calls) in the **same assistant turn** as `invoke_subagent` or `send_message`
  for an `inherit` subagent: tool calls emitted in the same turn run
  concurrently in the shared workspace. End the turn and wait for its reply
  before inspecting git state or files.
- **Bounded parallelism & concurrency boundaries:**
  - In **normal mode** with `Workspace: "share"` / `isolation: "worktree"`, the
    dispatcher may run **up to 2–3 independent subagents concurrently** (for
    example, unrelated issues with disjoint files, sibling PR `main`-sync
    passes, or asynchronous clean-context PR reviews while advancing to the next
    independent task), keeping container load within the Huddle sandbox's `2.0`
    vCPU / `8 GiB` memory budget.
  - Serialize heavy resource consumers (such as `mise run scan` / Trivy scans or
    large builds), and **never** run dependent tasks (`Depends on`,
    `Blocked by`, overlapping files, or stacked child PRs whose parent stacking
    preconditions have not yet passed) concurrently.
  - In **release mode** (or any `Workspace: "inherit"` run), run strictly **1
    subagent at a time**.
- **Clean up finished subagents and worktrees:** Once a PR reaches the
  [Human intervention gate](pull-requests.md#4-human-intervention-gate) (or a
  task aborts) and its review subagent has posted its comment, terminate
  unneeded subagents (`manage_subagents` with `Action: "kill"` in Antigravity)
  when no further follow-up is expected so idle subagents and worktrees do not
  accumulate across long queues or stacks. Killing a `Workspace: "share"`
  subagent automatically removes its temporary worktree directory and
  auto-created `subagent-*` branch while preserving any named feature branch
  (`git checkout -b ...`) in the shared `.git` repo; delete any unpushed local
  branch from an aborted task (`git branch -D ...`) and kill finished review
  subagents immediately after they post. Keep an authoring subagent alive only
  until its PR clears any bounded auto-fix cycle at the human gate.
- **Unambiguous `send_message`-only prompts:** When prompting an Antigravity
  subagent for a reply that requires no workspace reads or shell commands (for
  example, a status check or reflection), never instruct
  `"Do NOT call any tools"`, which conflicts with `<subagent_reminder>`
  requiring `send_message` to reply. Instead write:
  `"Do not read files or run commands; reply only via send_message."`

## 2. Input modes

The dispatcher supports two ways of defining the task queue:

1. **Explicit list mode:** The human specifies an ordered list of issue numbers
   and/or PR numbers to work through sequentially (e.g. "Tackle issues 52, 55,
   and 60"). The dispatcher processes only the specified items,
   topological-sorting any detected prerequisite dependencies (issues or queued
   parent PRs; see
   [Dependency detection and ordering](#dependency-detection-and-ordering))
   ahead of their dependents while preserving the given order for independent
   items. Dependent items remain subject to the
   [stacking preconditions](#stacking-preconditions).
2. **Open-ended queue mode (default allowed task pool):** When asked to work
   through available tasks without an explicit list, the dispatcher queries
   GitHub for eligible work using the following baseline task pool
   (`Provisional` — subject to future tuning), inspecting individual issues and
   PRs with `gh issue view <id> --json ...` and `gh pr view <pr> --json ...` so
   raw Markdown bodies, checklists, comments, and reviewed commit OIDs are not
   wrapped or truncated:
   - **Open PRs with human-approved review action items:** Open pull requests
     that have pending action items from a review (by a human or an agent review
     subagent) where a human has reviewed the findings and explicitly approved
     acting on them or provided direction (satisfying the
     [Human intervention gate](pull-requests.md#4-human-intervention-gate), e.g.
     via a human comment/review direction or the `approved` label, verified via
     `gh pr view <pr> --json title,body,headRefOid,comments,reviews,labels`).
     Note that a brief human go-ahead (such as `"take care of the nits"`)
     requires resolving all `Blocking` items alongside approved
     `Suggestion / Nit` items per
     [Follow-up and re-review workflow](pull-requests.md#5-follow-up-and-re-review-workflow).
     PRs awaiting initial human review after an agent review (or single bounded
     auto-fix re-review) comment remain gated and must not be picked up
     automatically—**except** for a dispatcher-initiated `origin/main` sync pass
     when a merged PR turns an open sibling PR from the queue `CONFLICTING` (see
     [Sibling PR `main`-sync passes](#sibling-pr-main-sync-passes)).
   - **Failing or unsynced Renovate dependency PRs:** Open PRs authored by
     Renovate (`app/renovate` or `renovate[bot]`) whose CI checks are failing or
     whose manifest changed without a matching lockfile update per
     [Handling Renovate dependency PRs](pull-requests.md#handling-renovate-dependency-prs).
     Passing Renovate PRs with synced lockfiles are left for human merge without
     agent intervention.
   - **Ready GitHub issues:** Open issues
     (`gh issue list --state open --json number,title,author,labels`, inspected
     with `gh issue view <id> --json title,body,labels,comments,author`; never
     combine `--comments` with `--json`) that meet **all** of the following
     criteria:
     - **Not awaiting refinement or externally blocked:** Exclude issues
       carrying `needs-refinement`. Exclude issues carrying `blocked` **unless**
       all of their declared blockers are prerequisite repository issues
       (`#<id>` via `Depends on #<id>` or `Blocked by #<id>` per
       [Declaring dependencies](issues.md#declaring-dependencies)) that are
       either closed/merged into `main`, ready in the current queue, backed by a
       parent PR queued in the current run, or already backed by a qualifying
       open PR (satisfying the
       [stacking preconditions](#stacking-preconditions)). Issues with external
       blockers, pending decisions, unidentifiable blockers, or unready
       prerequisites remain excluded.
     - **Human-created or human-approved:** Exclude **all** bot-authored issues
       (where `author.is_bot` is `true` or `author.login` is a bot/app account
       such as `app/wj44-agents` or `app/renovate`, regardless of whether they
       carry `proposal`), as well as any issue carrying the `proposal` label,
       **unless** the issue carries the `approved` label (and, for `proposal`
       issues or issues with an approval checklist, has
       `- [x] Approved to proceed` checked in the issue body per
       [Human approval gate](issues.md#human-approval-gate)).
     - **Not reserved for humans:** Exclude issues explicitly marked for human
       action (e.g. a `For human:` title prefix).
     - **No existing open PR:** Exclude any issue that already has an open pull
       request referencing or closing it (`gh pr list --state open`).

### Dependency detection and ordering

In both **open-ended queue mode** and **explicit list mode**, the dispatcher
resolves dependencies across tasks before execution:

- **Detection sources:** Use both explicit issue body markers
  (`Depends on #<id>` or `Blocked by #<id>`) and scope/description inference
  (for example, when an issue depends on an issue linked to a queued PR or
  builds on files or behavior introduced by another open issue or open unmerged
  PR).
- **Topological ordering:** Sort prerequisite tasks—including both prerequisite
  issues and queued parent PRs (for example, an open parent PR queued for
  human-approved review action items whose linked issue or changes a dependent
  issue relies on)—ahead of dependent issues (`A` before `B` before `C`),
  preserving explicit list order or priority for independent items.
- **Unresolvable or merged prerequisites:**
  - If all prerequisites of an issue are already closed and merged into `main`,
    no stacking is needed—run the issue directly off `main`.
  - If an issue has a dependency cycle or depends on an open issue that is
    unready (`needs-refinement`, unapproved `proposal` or bot-authored issue,
    `For human:`, or externally `blocked`)—or omitted from an explicit
    list—without a queued or qualifying open PR, skip the dependent issue and
    record the reason in the [completion summary](#5-completion-summary).

### Stacking preconditions

In both **open-ended queue mode** and **explicit list mode**, a dependent issue
may be stacked onto a parent issue's unmerged PR (whether opened or updated
earlier in the same dispatcher run, or pre-existing from an earlier run) **only
when all** of the following conditions hold (evaluated against the parent PR's
post-update CI and review when that parent PR was queued and updated earlier in
the same run):

1. **Single unmerged parent (linear chain only):** The dependency forms a
   single-parent linear chain (`main <- A <- B <- C`). Multi-parent (diamond)
   dependencies that would require merging two or more unmerged PR branches are
   skipped until at most one parent PR remains unmerged.
2. **Open parent PR:** The parent issue has an open pull request
   (`gh pr view <parent-pr> --json state,baseRefName,headRefName,headRefOid,reviews,comments`
   shows `state: "OPEN"`).
3. **Passing local verification and CI checks:** The parent PR passed local
   verification and has passing CI checks (`gh pr checks <parent-pr>` passes
   with no failing or pending checks; wait for in-progress checks triggered on
   the parent PR's latest commit, including on intermediate stacked PRs since
   `pull_request` CI triggers across all base branches).
4. **Clean-context agent review passed:** The parent PR's clean-context agent PR
   review comment for its latest commit (`headRefOid`, verified against
   `reviews[].commit.oid` and `comments` from
   `gh pr view <parent-pr> --json ...` so reviewed commit OIDs and full Markdown
   bodies are preserved)—or for its pre-sync feature commit when subsequent
   commits are solely a simple `origin/<base>` sync merge qualifying to skip
   re-review per
   [Follow-up and re-review workflow](pull-requests.md#5-follow-up-and-re-review-workflow)—has
   been posted with **no** `Blocking` or `Needs Human Decision` findings
   (`Suggestion / Nit` findings or an explicit pass are permitted). Do **not**
   stack onto a parent PR whose substantive re-review was skipped due to `429`
   quota exhaustion.

If a parent task (issue or queued PR) fails, is skipped, has failing CI, or
still has `Blocking` or `Needs Human Decision` findings after its PR review
(including any single bounded auto-fix and re-review permitted by the
[Agent PR review workflow](pull-requests.md#agent-pr-review-workflow)), skip all
dependent issues in the queue for the remainder of the run and report them in
the [completion summary](#5-completion-summary).

## 3. Git isolation between tasks

Tasks must be strictly isolated at the git level. In normal mode
(`Workspace: "share"` / `isolation: "worktree"`), each subagent operates in its
own isolated git worktree rather than mutating the dispatcher's checkout; in
release mode (`Workspace: "inherit"`), tasks run sequentially against the shared
workspace repository (never run dispatcher `git` or file commands—or batch
`run_command` in the same assistant turn as `invoke_subagent` or
`send_message`—while an `inherit` subagent is active):

1. **Prepare a clean base before every task:** Between tasks (or in the main
   checkout when no `inherit` subagent is active), verify that the working tree
   and index are completely clean (`git status --porcelain` is empty):
   - **Independent task (no unmerged parent PR):** Reset to the latest default
     branch and prune stale remote-tracking refs:

     ```bash
     git checkout main && git fetch --prune && git pull --ff-only
     ```

   - **Stacked dependent task (all stacking preconditions met):** Prune stale
     remote refs, fetch, check out, and update the parent PR's branch instead of
     `main` (pulling explicitly from `origin` so local branches pushed without
     `-u` do not fail for missing upstream tracking, or using
     `gh pr checkout <parent-pr>`). If a finished `Workspace: "share"` subagent
     still holds `<parent-branch>` checked out in its worktree, terminate that
     subagent (`manage_subagents` `Action: "kill"`) before running
     `git checkout <parent-branch>` in the main checkout—or keep the main
     checkout on `main` and instruct the stacked `share` subagent to branch
     directly from `origin/<parent-branch>`
     (`git fetch origin && git checkout -b <type>/<issue-id>-<slug> origin/<parent-branch>`):

     ```bash
     git fetch --prune origin && git checkout <parent-branch> && git pull --ff-only origin <parent-branch>
     ```

   - **Clean up merged local task branches safely:** After switching to the
     clean base (`main` or `<parent-branch>`), terminate any finished worktree
     subagents (`manage_subagents` `Action: "kill"`) so `git branch -d` / `-D`
     is not blocked by an active worktree checkout, then delete accumulated
     local task branches while preserving `main`, the active base branch, any
     branch with an `OPEN` PR, and any unmerged `<parent-branch>` needed later
     in the queue:
     - Delete branches already reachable from `main`
       (`git branch --merged main`, excluding `main` and the current branch)
       using `git branch -d <branch>`.
     - Because squash-merged PRs do not appear in `git branch --merged main`,
       query open and merged PRs in bulk rather than running per-branch
       `gh pr list --head <branch>` loops across accumulated local branches
       (always specifying `--state` and `--limit`, since bare `gh pr list`
       defaults to `--state open --limit 30`):
       - `gh pr list --state open --limit 100 --json headRefName,headRefOid`
       - `gh pr list --state merged --limit 100 --json headRefName,headRefOid`
     - Delete a remaining local `<branch>` via `git branch -D <branch>` **only**
       when it has no `OPEN` PR in the open set, matches a `MERGED` PR in the
       merged set (only falling back to a targeted
       `gh pr list --head <branch> --state all --json state,headRefOid` lookup
       for a candidate branch when >100 merged PRs exist, or leaving unverified
       branches untouched), and has no unpushed local commits (its local tip
       matches or is an ancestor of `origin/<branch>` if present, or the merged
       PR's `headRefOid` when the remote branch has been pruned).

2. **Dedicated branch per task:**
   - **For independent issues:** The task subagent creates its own branch from
     the updated `main` branch (`<type>/<issue-id>-<slug>` per
     [Commits and branches](commits.md#branch-naming)), implements and verifies
     the change, commits, pushes, and opens a pull request targeting `main`
     following [Pull requests](pull-requests.md) and
     [`.github/PULL_REQUEST_TEMPLATE/agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md).

   - **For stacked dependent issues:** The task subagent creates its own branch
     from `<parent-branch>` (`<type>/<issue-id>-<slug>`), implements and
     verifies the change, commits, pushes, and opens the pull request targeting
     the parent branch using
     [`.github/PULL_REQUEST_TEMPLATE/agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md):

     ```bash
     gh pr create --base <parent-branch> ...
     ```

     The PR body must note `Stacked on #<parent-pr>` (or
     `Depends on #<parent-pr>`) prominently in both `Context & Goal` and
     `Reviewer Hand-off` per
     [Stacked pull requests](pull-requests.md#stacked-pull-requests).

   - **For existing PRs (human-approved review follow-ups or Renovate PRs
     needing intervention):** When the original authoring subagent is still
     available in the active session, reuse/message it (`send_message` in
     Antigravity / resuming the task agent in Claude Code) to preserve task
     context and save tokens; spawn a new task subagent only when picking up the
     PR in a separate/later session where the original authoring subagent is no
     longer present. When starting fresh in a worktree (or after an `inherit`
     dispatcher switched branches), begin with
     `git fetch origin && gh pr checkout <pr-number>` (avoiding `gh pr checkout`
     if that branch is already checked out in another active worktree—reuse the
     owning subagent or kill the stale worktree first), address the feedback per
     [Follow-up and re-review workflow](pull-requests.md#5-follow-up-and-re-review-workflow)
     (or resolve the Renovate CI/lockfile issue), verify formatting/linting,
     update the PR body if needed, commit, push, wait for post-push CI on the
     updated commit, and spawn a **separate clean-context subagent** for the
     re-review.
3. **Post-PR review hand-off:** After opening or updating a PR, the task
   subagent follows the
   [Agent PR review workflow](pull-requests.md#agent-pr-review-workflow)
   (including at most one bounded auto-fix and re-review cycle when an initial
   PR review has `Needs Human Decision: None` and flags `Blocking` findings
   and/or clear, low-impact, in-scope `Suggestion / Nit` items, while keeping
   out-of-scope, architectural, or trade-off suggestions gated on human approval
   or deferred to follow-up `Agent Proposal` issues). Once the applicable review
   (or single bounded re-review) comment is posted on that PR, work on that PR
   stops (respecting the
   [human intervention gate](pull-requests.md#4-human-intervention-gate) for
   that PR) and control returns to the dispatcher to evaluate whether any queued
   dependents may stack onto it (per
   [stacking preconditions](#stacking-preconditions)), reset to the next task's
   base branch, and continue.

### Sibling PR `main`-sync passes

When a human merges an earlier PR into `main` mid-run (observed when resetting
to `main` between tasks), open sibling PRs from the same queue run may become
`CONFLICTING`—causing GitHub Actions to skip `pull_request` workflows on those
PRs. Between tasks (with no `inherit` subagent active), the dispatcher may check
open sibling PRs via
`gh pr view <pr> --json state,baseRefName,headRefOid,mergeable,mergeStateStatus`
(waiting briefly if `mergeable` is still `UNKNOWN`).

If an open sibling PR reports `mergeable == "CONFLICTING"` or
`mergeStateStatus == "DIRTY"`, the dispatcher is permitted—even though the PR
was otherwise waiting at the human intervention gate—to initiate an
`origin/main` (or `origin/<base>`) sync pass:

- Reuse the original authoring subagent via `send_message` when available in the
  active session (or spawn a fresh subagent if not; when reusing a `share`
  authoring subagent whose worktree already holds the PR branch, run
  `git fetch origin && git status --porcelain` rather than re-running
  `gh pr checkout` from another worktree).
- If starting from a fresh checkout/worktree (ensure no other active worktree
  holds the branch), start with
  `git fetch origin && gh pr checkout <pr-number>`, then merge `origin/<base>`
  cleanly without expanding scope, verify `git diff origin/<base>...HEAD`,
  re-run local verification (`mise x -- lefthook run format-check`,
  `mise x -- lefthook run lint-check`, `bin/harness.py render --check` when
  relevant, and task-specific tests), update `Reviewer Hand-off` /
  `Acceptance Criteria & Verification` (`gh pr edit <pr> --body-file -`) only if
  conflict resolution changed behavior or verification notes, commit, push, and
  wait for post-push CI on the merge commit (`bin/wait-pr-checks [<pr>]` and
  `gh run list --commit <headRefOid> ...`) per
  [Follow-up and re-review workflow](pull-requests.md#5-follow-up-and-re-review-workflow).
- **Skip re-review on simple sync merges:** If the PR already had a
  clean-context review on its pre-sync commit and the sync commit(s) solely
  merged `origin/<base>` with simple, mechanical conflict resolution (unchanged
  feature behavior in `git diff origin/<base>...HEAD` and no bundled follow-up
  edits), **do not spawn a clean-context re-review subagent**—stop directly at
  the [human intervention gate](pull-requests.md#4-human-intervention-gate) (or
  re-evaluate [stacking preconditions](#stacking-preconditions) if it is a
  parent PR).
- **Require re-review on substantive merges or combined changes (with `429`
  fallback):** Spawn a **separate clean-context subagent** for a re-review of
  the updated `headRefOid` when the PR had no prior clean-context review,
  conflict resolution modified feature logic or required non-trivial adaptation
  (such as renumbering a Decision Record across cross-references or adapting
  code to upstream API changes), or review follow-up edits were bundled with the
  merge, then return to the
  [human intervention gate](pull-requests.md#4-human-intervention-gate) (or
  evaluate [stacking preconditions](#stacking-preconditions) if it is a parent
  PR). If that re-review subagent fails due to quota exhaustion
  (`429 RESOURCE_EXHAUSTED`), follow the fallback in
  [Follow-up and re-review workflow](pull-requests.md#5-follow-up-and-re-review-workflow)—report
  the PR at the human gate with its prior review, CI status on `headRefOid`, and
  `git diff` scope (and skip any dependents awaiting that substantive re-review
  in the [completion summary](#5-completion-summary)) rather than blocking
  silently.

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
  dispatcher so it can skip any queued dependents of that task (recording them
  in the [completion summary](#5-completion-summary)) and proceed to the next
  independent task in the queue.

## 5. Completion summary

Once every item in the explicit list or eligible open-ended queue has been
processed, the dispatcher stops and reports a concise summary to the human
covering:

- Pull requests opened or updated (with PR numbers, links, and any stack links /
  merge order such as `stacked on #<parent-pr>`).
- Tasks skipped or blocked (with issue/PR numbers and the reason, including
  dependents skipped due to failed parents, `Blocking` / `Needs Human Decision`
  review findings, `429` quota-exhausted substantive re-reviews, or multi-parent
  dependencies).
