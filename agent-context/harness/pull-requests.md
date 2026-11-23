# Pull requests

Pull requests in this repository are typically created by autonomous agents and
reviewed by humans who may have had zero prior involvement in the task.
Reviewers need to be able to evaluate the PR and make a confident decision in 3
to 5 minutes (e.g. during a coffee break) without digging through raw logs or
session transcripts.

Occasional PRs are also opened by humans; the same branch and title conventions
apply, with a lighter-weight template.

## Branch naming

Pull requests must be opened from branches following the conventions documented
in [Commits and branches](commits.md#branch-naming) (`<type>/<name>`).

## Title format

PR titles must follow the [Conventional Commits](commits.md) format:

```text
<type>[optional scope]: <description>
```

Example: `feat(setup): split setup script into system and user scripts (#11)`

## Templates

Templates live in `.github/PULL_REQUEST_TEMPLATE/`:

- [`pull_request_template.md`](../../.github/PULL_REQUEST_TEMPLATE/pull_request_template.md)
  (Default): Automatically loaded by the GitHub web UI for human-authored PRs.
  Kept minimal (`Context & Changes`, `Notes / Verification`).
- [`agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md): Used by agents
  creating PRs via `gh pr create`. Structured as a self-contained review packet
  for an uninvolved human reviewer.

## Agent PR requirements

When creating or updating a pull request via `gh pr create` or `gh pr edit`,
read and follow
[`.github/PULL_REQUEST_TEMPLATE/agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md)
(use `--body-file -` for multi-line Markdown bodies to preserve formatting and
checklists cleanly):

- **Context & Goal:** 1–2 sentences summarizing the problem and goal, linking
  relevant issues (`Closes #<id>`, `Fixes #<id>`, or `Refs #<id>`).
- **Summary of Changes:** Concise bullets grouped by behavior or concept rather
  than repeating the file-by-file diff.
- **Reviewer Hand-off:** Context transfer for a reviewer who was not in the
  session — non-obvious decisions/trade-offs, files needing close scrutiny, and
  any caveats, follow-up `Agent Proposal` issues, or items untestable in the
  sandbox (guard against the "looks done" trap; state "None" if none).
- **Acceptance Criteria & Verification:** Mirror the issue's acceptance criteria
  checklist (`- [x] ...`) and briefly note how behavior was verified in practice
  (do not pad with boilerplate `format-check`/`lint-check` checkboxes or raw
  terminal logs).
- **Stable references & concise PR bodies:** Cite file paths, Markdown headings,
  symbol names, and commands run rather than volatile line numbers or mutable
  merge commit SHAs, which go stale across upstream sync merges or follow-up
  commits. Keep the PR body concise and scannable; put long, evolving
  walkthroughs or reference tables in a dedicated PR comment instead.

## Stacked pull requests

When implementing a dependent issue stacked on an unmerged parent PR (see
[Task queue orchestration](task-queue.md)):

- **Branch & base target:** Using the `<parent-branch>` and `#<parent-pr>`
  passed in the hand-off prompt, create the feature branch
  (`<type>/<issue-id>-<slug>`) from `<parent-branch>` rather than `main`, and
  open the PR targeting the parent branch:

  ```bash
  gh pr create --base <parent-branch> ...
  ```

- **Document the stack in the PR body:** Note `Stacked on #<parent-pr>` (or
  `Depends on #<parent-pr>`) prominently in both **Context & Goal** (including
  the merge order) and **Reviewer Hand-off** (noting base branch
  `<parent-branch>` and any assumptions inherited from `#<parent-pr>`). Keep
  **Acceptance Criteria & Verification** scoped to the child issue.

- **Independent review & gate:** Each PR in a stack follows the
  [Agent PR review workflow](#agent-pr-review-workflow) and stops at the
  [Human intervention gate](#4-human-intervention-gate) once its review (or
  single bounded re-review) comment is posted.

## Agent PR review workflow

Pull requests created by an autonomous agent must be reviewed by a second agent
running with a clean context before a human makes the final merge or revision
decision. Reviewing in the authoring agent's existing session risks confirmation
bias from accumulated reasoning, while waiting for a human to manually kick off
a review adds friction to the coffee-break review workflow.

### 1. Post-PR review hand-off

Immediately after opening a pull request (`gh pr create`) (or once initial CI
checks pass when waiting for CI before review), the authoring agent must spawn a
**separate** subagent with a clean context (e.g. `invoke_subagent` in
Antigravity or the `Agent` tool in Claude Code) to review the PR. In normal-mode
repositories where harness files (`AGENTS.md`, `.agents/`, `hooks/post-edit`,
`agent-context/harness/`) are tracked in git, launch the review subagent in an
isolated worktree (`Workspace: "share"` in Antigravity; `isolation: "worktree"`
in Claude Code) so the review never collides with the dispatcher's checkout and
can run asynchronously; in release-mode repositories (where
`RELEASE_EXCLUDED_PATHS` are gitignored and absent in fresh worktrees), use
`Workspace: "inherit"`. Whenever waiting for post-push CI (after `gh pr create`
or on follow-up commits from a bounded auto-fix, human-directed follow-up, or
base-branch sync merge), follow the post-push CI check wait procedure in
[Follow-up and re-review workflow](#5-follow-up-and-re-review-workflow) before
spawning a fresh, **separate** clean-context review or re-review subagent (when
required—pure simple base-branch sync merges on already-reviewed PRs may skip
re-review per that section) so the authoring agent never reviews its own
changes:

- Pass **only** the PR number and instructions to read and follow the review
  guide in `agent-context/harness/pull-requests.md`.
- Do **not** pass the authoring agent's implementation reasoning, session
  transcript, or justifications, so the reviewing subagent evaluates the PR on
  its own merits.
- **Reviewer subagent type (Antigravity network boundary):** Spawn the
  clean-context PR reviewer in Antigravity with `TypeName: "self"`, **never**
  `TypeName: "research"`. Antigravity's `research` subagent runs in a
  networkless sandbox and cannot run `gh pr view`, `gh pr diff`, `gh pr checks`,
  or `gh pr review`. Because `self` retains edit and git tools, the reviewing
  subagent must strictly honor the
  [read-only boundary](#3-review-output-format--delivery). (In Claude Code, use
  a general-purpose `Agent` with `gh` access.)
- **Clean up finished review subagents:** Once the review comment is posted and
  no further review messages are needed, the parent/dispatcher should terminate
  the finished review subagent (`manage_subagents` with `Action: "kill"` in
  Antigravity) so idle subagents and `Workspace: "share"` worktrees do not
  accumulate across long stacks.

### 2. Review rubric (points of attention)

The reviewing subagent inspects the PR metadata, description, comments, and
reviews
(`gh pr view <pr> --json title,body,state,baseRefName,headRefName,headRefOid,mergeable,mergeStateStatus,statusCheckRollup,comments,reviews,labels`),
the diff (`gh pr diff <pr>`), status checks (`gh pr checks <pr>`), and the
linked issue (`gh issue view <issue> --json title,body,labels,comments,author`)
against four points of attention:

- **Structured `gh` flags and pending check exit codes:** Using `--json` avoids
  terminal line-wrapping and truncation while exposing `headRefOid` and reviewed
  commit OIDs via `reviews[].commit.oid`. Never combine `--comments` with
  `--json` on `gh issue view` (they are mutually exclusive and fail with
  `specify only one of --comments or --json`; `--json ...,comments,...` already
  returns full comments). When inspecting CI, `gh pr checks <pr>` exits with
  code `8` while checks are still pending (and code `1` on failures or
  `no checks reported`), so do **not** chain `gh pr checks <pr>` with `&&` in a
  shell command—run it standalone, use
  `gh pr checks <pr> --json bucket,name,state,workflow`, or inspect
  `statusCheckRollup` (accounting for brief post-push/post-create GitHub Actions
  registration lag, or `CONFLICTING` PRs where GitHub Actions skips
  `pull_request` workflows).
- **Verify CI ran on `headRefOid`:** Because `gh pr checks <pr>` and
  `statusCheckRollup` do not expose the tested commit SHA, also run
  `gh run list --commit <headRefOid> --json databaseId,workflowName,status,conclusion,headSha`
  to confirm the passing workflow runs match the PR's latest `headRefOid` rather
  than an earlier commit.
- **`commitlint` verification:** Because `lefthook run format-check`,
  `lefthook run lint-check`, and CI do not invoke `commitlint` (it runs only in
  the `commit-msg` hook), verify commit messages on the PR branch with
  `mise x -- commitlint --from origin/<baseRefName> --to <headRefOid>` when both
  refs exist locally; if local objects are absent (without running
  `git checkout`), inspect commit subjects/footers and the PR title via
  `gh pr view <pr> --json commits,title` against
  [Commits and branches](commits.md) and note whether `commitlint` was skipped.
- **Inspecting PRs with merge commits via `gh pr diff`:** `gh pr diff <pr>`
  already computes the net diff against the base branch; on PRs containing
  `main`/`<base>`-sync merge commits, rely on `gh pr diff <pr>` and limit
  `git show <merge-sha> -- <path>` to conflict-resolved files rather than
  re-auditing merged upstream commits.
- **Verify workspace `HEAD` and clean tree before local checks:** Because the
  reviewing subagent is strictly read-only—and a `Workspace: "share"` /
  `isolation: "worktree"` reviewer starts at the parent session's `HEAD` rather
  than the PR branch when spawned from a different checkout—check
  `git rev-parse HEAD` against the PR's `headRefOid` and verify
  `git status --porcelain` is empty **before** reading workspace files or
  running read-only local checks (`lefthook`, AST checks,
  `bin/harness.py render --check`, etc.). If `HEAD` does not match `headRefOid`
  or the working tree is dirty, do **not** trust local workspace reads or
  checks; inspect the PR via `gh pr diff <pr>` and
  `git show <headRefOid>:<path>` (when `<headRefOid>` is present locally)
  instead, and note any local checks skipped due to the workspace mismatch.
- **Re-reviews:** Verify prior findings are resolved on the latest `headRefOid`
  without regressions and confirm the PR body matches the current diff.

1. **Acceptance criteria & scope boundaries:** Verify that every acceptance
   criterion on the linked issue is genuinely satisfied, and that the diff stays
   strictly within the declared scope and non-goals without unrelated
   refactoring (do not ask the author to expand scope for untouched files; treat
   separate cleanup observations strictly as non-blocking `Suggestion / Nit`
   items). If a human explicitly approved carving out or deferring part of an
   issue (documented in issue comments or refined scope), accept `Refs #<id>`
   (instead of `Closes #<id>`) alongside an unchecked `- [ ]` criterion that
   clearly explains the deferral in `Acceptance Criteria & Verification` and
   `Reviewer Hand-off`; silently leaving criteria unmet without explicit human
   approval remains `Blocking`. For stacked PRs (`--base <parent-branch>`),
   `gh pr diff <pr>` scopes to the child PR's incremental changes—evaluate only
   the child issue's diff and acceptance criteria.
2. **Conventions & Decision Records:** Verify that changes comply with
   [`agent-context/`](..) guidelines and active [`decisions/`](../../decisions/)
   and [`decisions/harness/`](../../decisions/harness/) records (`Standard` and
   `Locked` finality levels; see [Decisions](decisions.md)). Confirm that a new
   or updated Decision Record is included (and indexed in
   [`decisions/README.md`](../../decisions/README.md) or
   [`decisions/harness/README.md`](../../decisions/harness/README.md)) if the PR
   introduces or changes a convention, tool, or workflow.
3. **Correctness, edge cases & security:** Check logic, error handling, test
   coverage, edge cases, and security/credential boundaries. Rather than
   inspecting isolated diff hunks alone, **cold-read each modified section in
   full** (or the full file for short docs and configs) for standalone clarity,
   awkward placeholders, contradictory surrounding text, or broken flow.
4. **PR hand-off accuracy:** Verify that the `Reviewer Hand-off` section is
   accurate and guards against the "looks done" trap (any stubbed, deferred, or
   sandbox-untestable items are explicitly called out, and required decision
   records or follow-up proposals are present). For stacked PRs, also verify
   that the PR targets the intended `<parent-branch>` (`baseRefName` in
   `gh pr view <pr> --json baseRefName`) and accurately notes the stack
   relationship (`Stacked on #<parent-pr>` or `Depends on #<parent-pr>`) in both
   `Context & Goal` and `Reviewer Hand-off`.

### 3. Review output format & delivery

- **Read-only boundary:** The reviewing subagent only reports findings—it must
  **not** modify files, create commits, push changes, or merge the PR (including
  running `git checkout` or `gh pr checkout`).
- **Delivery mechanism:** The reviewing subagent posts a concise, structured
  review comment on the PR via `gh pr review <pr> --comment` (preferred, as it
  records the reviewed commit OID in `reviews[].commit.oid`, using
  `--body-file -` for longer reviews; or `gh pr comment <pr>`). Do **not** use
  `gh pr review --approve` or `--request-changes`, as all agents share the same
  GitHub App bot identity and GitHub disallows approving or requesting changes
  on one's own PR.
- **File paths in review comments:** Reference repository files and line numbers
  using backticked paths (e.g.
  `` `agent-context/harness/pull-requests.md#L112` `` or
  `` `agent-context/harness/pull-requests.md:112` ``) rather than relative
  Markdown links (`[file](path/to/file.md)`), which do not resolve reliably in
  GitHub PR review comments.
- **Severity grouping:** Group findings by severity so a human can scan the
  review quickly:
  - **`Blocking`** — Unmet acceptance criteria, defects, security/boundary
    violations, or broken conventions that must be resolved before merge.
  - **`Needs Human Decision`** — Ambiguities, design trade-offs, or scope
    questions requiring human judgment.
  - **`Suggestion / Nit`** — Non-blocking improvements or minor polish.
  - **Explicit pass:** If no issues are found, state an explicit pass confirming
    the rubric checks were verified.

### 4. Human intervention gate

- **Default stop after clean, human-decision, or out-of-scope reviews:** Once
  the initial review comment is posted on the PR, both the reviewing subagent
  and the authoring agent **must stop and wait for human intervention**
  whenever:
  - `Needs Human Decision` has any items (even if `Blocking` or
    `Suggestion / Nit` items are also present), or
  - `Blocking` is `None` and there are **no** qualifying clear, low-impact,
    in-scope `Suggestion / Nit` items (an explicit pass, or only out-of-scope,
    architectural, or design-trade-off `Suggestion / Nit` items). Neither agent
    may act on out-of-scope/non-trivial `Suggestion / Nit` items or unresolved
    design questions without human direction.
- **Bounded auto-fix for clear `Blocking` findings and qualifying
  `Suggestion / Nit` items (`Needs Human Decision: None`):** When—and **only**
  when—the **initial** clean-context review on a newly opened PR has
  **`Needs Human Decision`** empty (`None`) and flags one or more **`Blocking`**
  findings and/or clear, low-impact, in-scope **`Suggestion / Nit`** items
  (including when `Blocking` is `None` and only qualifying low-impact nits
  exist), the original authoring agent may execute **at most one** bounded
  auto-fix and clean-context re-review cycle within its active session:
  1. Resolve all flagged `Blocking` findings together with any clear,
     low-impact, in-scope `Suggestion / Nit` items (such as syncing an in-scope
     cross-reference, fixing a small edge case, or clarifying wording within the
     issue's target files).
  2. **Guardrails — do not expand scope unilaterally:** Leave untouched any
     `Suggestion / Nit` item that touches out-of-scope components or unrelated
     files, changes design or architecture, introduces non-trivial trade-offs,
     or conflicts with explicit issue non-goals. Leave those items for human
     judgment (note them in `Reviewer Hand-off` or track them as follow-up
     `Agent Proposal` issues per
     [Autonomous agent proposals](issues.md#autonomous-agent-proposals)).
  3. Re-run local verification (`mise x -- lefthook run format-check`,
     `mise x -- lefthook run lint-check`, and task-specific tests), sync the PR
     body via `gh pr edit <pr> --body-file -` if behavior, hand-off notes, or
     verification changed, commit, and push to the PR branch.
  4. Wait for CI checks on the updated commit (handling post-push registration
     lag as described in
     [Follow-up and re-review workflow](#5-follow-up-and-re-review-workflow)),
     then spawn **one** separate clean-context re-review subagent (passing only
     the PR number and instructions to read and follow
     `agent-context/harness/pull-requests.md`).
  5. **Hard stop after one re-review:** Once that single re-review comment is
     posted—whether it passes or still reports findings—both agents **must stop
     and wait for human intervention**. Never perform a second auto-fix attempt
     without explicit human approval.
- **Stop after human-directed follow-ups:** After any human-approved follow-up
  and its separate clean-context re-review comment are posted, both agents
  **must stop** and wait for further human direction.

### 5. Follow-up and re-review workflow

When resuming an open PR after human review feedback or executing the single
bounded auto-fix permitted by the
[Human intervention gate](#4-human-intervention-gate) (for `Blocking` findings
and/or clear, low-impact, in-scope `Suggestion / Nit` items while leaving
out-of-scope, architectural, or trade-off items untouched):

- **Who executes follow-ups (reuse authoring agent when available):**
  - For both the single bounded auto-fix loop and human-directed review
    follow-ups while the original authoring agent/subagent is still available in
    the active session, **reuse/message the original authoring agent**
    (`send_message` in Antigravity or resuming the task agent in Claude Code) to
    preserve task context and save tokens.
  - Spawn a new task subagent only when picking up an open PR in a separate or
    later session where the original authoring subagent is no longer present.
  - **Always start by syncing the PR branch:** When resuming an authoring
    subagent in an existing `Workspace: "share"` / `isolation: "worktree"`
    worktree that already has the PR branch checked out, run `git fetch origin`
    (fast-forwarding if `origin` advanced) and confirm `git status --porcelain`
    is clean. Otherwise—in a fresh worktree or an `inherit` checkout where the
    dispatcher may have switched branches—start with
    `git fetch origin && gh pr checkout <pr-number>` (after ensuring no other
    active worktree holds that branch) and confirm `git status --porcelain` is
    clean.
  - Always spawn a **separate clean-context subagent** for the PR review and any
    re-review itself so the authoring agent never reviews its own changes.
- **Interpreting brief human review directives:** When a human leaves a brief or
  general go-ahead on a reviewed PR (for example, `"take care of the nits"`,
  `"address the feedback"`, or `"fix these"`), resolve **all remaining
  `Blocking` findings** from the latest clean-context review **alongside** the
  approved or requested `Suggestion / Nit` items—unless the human explicitly
  instructs leaving specific `Blocking` items untouched. If unresolved
  `Needs Human Decision` items remain that the human comment did not answer,
  leave them unchanged and ask for clarification rather than guessing.
- **Keep the PR body synced:** Whenever follow-up commits resolve review
  findings, change behavior, clear earlier caveats, or add new verification
  steps, refresh the PR description (`Summary of Changes`, `Reviewer Hand-off`,
  and `Acceptance Criteria & Verification`) using
  `gh pr edit <pr> --body-file -` (preserving issue-closing keywords such as
  `Closes #<id>` and any `Stacked on #<parent-pr>` notes).
- **Wait for post-push CI (handling `headRefOid` lag, `CONFLICTING` skips, and
  registration lag):** Immediately after `git push` (or `gh pr create`), GitHub
  may briefly return stale metadata (`headRefOid` from the prior commit,
  `mergeable: "UNKNOWN"` or prior conflict state, or empty `statusCheckRollup`)
  and Actions may take several seconds to register check runs, causing
  `gh pr checks <pr> --watch` to exit with code `1` (`no checks reported`) or
  evaluate stale checks from an older commit. Moreover, GitHub Actions silently
  skips `pull_request` workflows on conflicted PRs, leaving `gh pr checks <pr>`
  reporting `no checks reported` indefinitely. When PR CI workflows apply to the
  PR's base branch (all PRs when `on.pull_request` is unfiltered):
  1. **Run `bin/wait-pr-checks [<pr>]`:** Polls
     `gh pr view <pr> --json number,headRefOid,baseRefName,mergeable,mergeStateStatus`
     and `gh pr checks <pr> --json bucket,name,state,workflow` with a short
     backoff until `headRefOid == $(git rev-parse HEAD)` and `mergeable` settles
     from `UNKNOWN`, aborts immediately with exit code `2` if
     `mergeable == "CONFLICTING"` or `mergeStateStatus == "DIRTY"`, waits for CI
     checks on `HEAD` to register, and then watches `gh pr checks <pr> --watch`
     until completion.
  2. **Merge `origin/<base>` immediately if `CONFLICTING` (exit code `2`):**
     **Stop waiting for skipped CI runs**. Fetch and merge the base branch
     (`git fetch origin <base> && git merge origin/<base>`, where `<base>` is
     `baseRefName`, e.g. `main`), resolve any conflicts cleanly without
     expanding scope, re-run local verification
     (`mise x -- lefthook run format-check`,
     `mise x -- lefthook run lint-check`, and task-specific tests), commit,
     push, and re-run `bin/wait-pr-checks [<pr>]` for the new `HEAD`.
  3. **Proceed only after all checks pass (exit code `0`):** (Only in downstream
     configurations that explicitly filter `pull_request` CI away from the PR's
     base branch should an agent rely on passing local verification instead of
     running `bin/wait-pr-checks [<pr>]` / `gh pr checks <pr> --watch`.)
- **When to skip clean-context re-review (simple base-branch sync merges):**
  After local verification and post-push CI pass on the new merge commit
  (`bin/wait-pr-checks [<pr>]` and
  `gh run list --commit <headRefOid> --json databaseId,workflowName,status,conclusion,headSha`
  confirming `headRefOid` matches the merge commit), you may **skip spawning a
  clean-context re-review subagent** and stop directly at the
  [Human intervention gate](#4-human-intervention-gate) when **all** of the
  following hold:
  1. The PR already received a clean-context review on its pre-sync feature
     commit (`reviews[].commit.oid` or prior review comment covering the
     pre-sync commit).
  2. The new commit(s) solely merge `origin/<base>` (`origin/main` or
     `origin/<parent-branch>`) and resolve only simple, mechanical conflicts
     (such as `decisions/*/README.md` table rows, additive lines kept from both
     branches, or adopting upstream `origin/<base>` updates) without altering
     feature behavior or scope in `git diff origin/<base>...HEAD`.
  3. No review follow-up edits or new feature/fix changes were bundled with the
     merge.
  4. The authoring or sync agent verified `git diff origin/<base>...HEAD`, ran
     local verification (`mise x -- lefthook run format-check`,
     `mise x -- lefthook run lint-check`, `bin/harness.py render --check` when
     relevant, and any applicable tests), synced the PR body only if
     verification notes or caveats changed, pushed, and confirmed CI passed on
     the merge commit's `headRefOid`.
- **When re-review is still required after a merge:** Always spawn a
  **separate** fresh clean-context re-review subagent after CI passes on
  `headRefOid` whenever:
  1. The PR has not yet received an initial clean-context review;
  2. Conflict resolution modifies feature logic, introduces non-trivial
     adaptation (such as renumbering a Decision Record across cross-references
     or refactoring code to match upstream API changes on `<base>`), or touches
     behavior outside mechanical conflict resolution; or
  3. The push combines review follow-up edits or new feature/fix changes
     alongside the merge.
- **Spawn a separate clean-context re-review only after CI passes (with `429`
  quota fallback):** Whenever re-review is required, wait until local
  verification and CI checks on the updated commit (`headRefOid`) pass, then
  spawn a **separate** fresh clean-context review subagent (passing only the PR
  number and `agent-context/harness/pull-requests.md` instructions), and stop at
  the [Human intervention gate](#4-human-intervention-gate) once the re-review
  comment is posted. If the clean-context **re-review** subagent fails due to
  quota or rate-limit exhaustion (`429 RESOURCE_EXHAUSTED`), do **not** block
  silently or retry indefinitely—stop at the
  [Human intervention gate](#4-human-intervention-gate) and report the PR
  explicitly with:
  - Its prior clean-context review status and reviewed commit OID,
  - The latest `headRefOid` and passing post-push CI status,
  - The scope of changes since the prior review
    (`git diff <prior-reviewed-oid>..HEAD` and `git diff origin/<base>...HEAD`),
    and
  - A note that clean-context re-review could not run due to `429` quota
    exhaustion and awaits human review or a manual re-review retry.

## Handling Renovate dependency PRs

Automated dependency updates are managed by Renovate.

Intervene on a Renovate PR only when CI fails or a lockfile is out of sync (for
example, a formatter/linter bump requires reformatting existing files, a new CVE
fails `mise run scan`, or another ecosystem lacks lockfile support):

1. **Check out the PR branch**: `gh pr checkout <pr-number>`.
2. **Resolve the failure**: Refresh lockfiles (`mise lock`) or formatting
   (`lefthook run format`) as needed, or investigate scan/lint errors.
3. **Verify locally**: Run
   `lefthook run format-check && lefthook run lint-check` (prefix with
   `mise x --` if `mise` shims are not on `PATH`; see
   [Formatting and linting](formatting-and-linting.md#formatting)), and
   `mise run scan` if security checks failed.
4. **Commit and verify CI**: Commit any fixes using Conventional Commits with
   type `chore` (e.g. `chore(deps): format for prettier update`), push, and
   confirm CI passes on the PR branch.
