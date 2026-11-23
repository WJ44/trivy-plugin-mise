# 019: Agent PR review process

- **Status:** Active
- **Finality:** Standard
- **Scope:** `agent-context/harness/pull-requests.md`,
  `agent-context/harness/task-queue.md`, `AGENTS.md`
- **Date:** 2026-11-23

## Context

Pull requests are primarily authored by autonomous agents and reviewed by humans
during short review windows. Having the authoring agent self-review within its
own session risks confirmation bias from accumulated reasoning, while requiring
a human to manually spin up a second session before reviewing adds friction. At
the same time, halting unconditionally after the first review when the reviewer
finds objective `Blocking` defects or straightforward, low-impact, in-scope
`Suggestion / Nit` polish with no open design questions
(`Needs Human Decision: None`) forces avoidable human approval round-trips (such
as `"take care of the nits"`) before the PR is merge-ready. Conversely,
requiring a fresh clean-context re-review subagent after every base-branch sync
merge—even when an already-reviewed PR only resolves mechanical merge conflicts
or merges an updated parent branch without altering feature behavior—adds
substantial latency and causes avoidable quota exhaustion (`429`).

## Decision

After opening a PR, the authoring agent spawns a **separate** subagent with a
clean context (passing only the PR number and review instructions) to perform an
independent, report-only review and post a structured review comment on the PR.
When an initial PR review has `Needs Human Decision: None` and flags `Blocking`
findings and/or clear, low-impact, in-scope `Suggestion / Nit` items (including
when `Blocking` is empty and only qualifying low-impact nits exist), the
original authoring agent may perform **at most one** bounded auto-fix and
separate clean-context re-review iteration (waiting for CI checks on the updated
commit before re-review). Out-of-scope, architectural, or design-trade-off
suggestions must still be left untouched (noted for human decision or deferred
to a follow-up `Agent Proposal` issue). Both agents stop and wait for human
direction immediately after the initial review whenever `Needs Human Decision`
is non-empty or only an explicit pass or non-qualifying (out-of-scope or
design-trade-off) `Suggestion / Nit` items are reported, and always after the
single bounded re-review or any human-directed follow-up re-review. For
follow-up implementation (both the bounded auto-fix and human-directed revisions
while the authoring agent remains available in the active session),
reuse/message the original authoring agent to preserve context and save tokens,
spawning a new task agent only in a separate or later session when the original
subagent is gone—while **always** spawning a separate clean-context subagent for
the PR review or re-review itself.

For base-branch sync merges (`origin/main` or `origin/<parent-branch>`), skip
spawning a clean-context re-review subagent and stop directly at the human
intervention gate when an already-reviewed PR merges only simple, mechanical
conflicts with unchanged feature behavior and scope in its net diff and passes
local verification plus post-push CI on the merge commit. Still require a
separate clean-context re-review whenever conflict resolution modifies feature
logic, requires non-trivial adaptation (such as renumbering a Decision Record or
refactoring code to match upstream API changes), or bundles review follow-up or
feature/fix edits alongside the merge. If a required re-review subagent fails
due to quota exhaustion (`429`), stop at the human intervention gate and report
the PR with its prior review status, CI status on the latest commit, and
`git diff` scope rather than blocking silently.

## Options Considered

- **Clean-context subagent + `agent-context/harness/pull-requests.md` rubric
  (chosen):** Works natively across supported agent CLIs, isolates the reviewer
  from the author's reasoning history, and triggers automatically when a PR is
  opened.
- **Reuse authoring agent for follow-ups + separate clean-context subagent for
  reviews/re-reviews (chosen):** Preserves task context and saves tokens during
  revisions without compromising review independence, reserving fresh
  implementer subagents for later sessions where the original subagent is
  absent.
- **Always spawn a new implementer subagent for every follow-up:** Rejected for
  active sessions because re-reading the codebase and PR context wastes tokens
  when the original authoring subagent already holds that context.
- **One bounded auto-fix + re-review when `Needs Human Decision: None` for
  `Blocking` findings and clear, low-impact, in-scope `Suggestion / Nit` items
  (chosen for the intervention gate):** Eliminates avoidable human round-trips
  for both objective bugs and routine in-scope polish while preserving strict
  scope guardrails (out-of-scope, architectural, or trade-off suggestions remain
  gated on human approval or follow-up proposals) and the one-iteration hard
  stop.
- **One bounded auto-fix + re-review only when `Blocking` is present and
  `Needs Human Decision: None` (previous rule):** Eliminated round-trips for
  `Blocking` defects, but still forced avoidable `"take care of the nits"` human
  comments and extra CI/re-review cycles when initial reviews surfaced only
  clear, low-impact, in-scope `Suggestion / Nit` polish.
- **Strict stop after first review regardless of findings (original baseline):**
  Rejected after observing avoidable human round-trips when clear `Blocking`
  bugs or low-impact nits had `Needs Human Decision: None`.
- **Skip clean-context re-review on simple, mechanical base-branch sync merges
  (chosen for sync merges):** Eliminates redundant subagent turns and quota burn
  when pre-sync behavior is unchanged and local checks plus CI pass on the merge
  commit, while preserving re-review for substantive conflict adaptation or
  bundled edits and failing safely to the human gate on `429` quota exhaustion.
- **Diff-only lightweight re-review on every merge:** Spawning a scoped
  re-review subagent to inspect only the merge commit and net diff saves
  execution time per review, but still consumes an extra subagent turn and quota
  for trivial conflict merges.
- **Portable skill (`.agents/skills/` + `.claude/skills/`):** Rejected because
  PR review is already part of the standard PR workflow in
  `agent-context/harness/pull-requests.md`.
- **Wrapper script (`bin/review-pr`):** Rejected because it requires hardcoding
  or detecting which agent CLI and model to invoke headlessly.
- **Manual review trigger only:** Rejected because it requires a human to
  manually launch a second session for every PR.

## Consequences

Every agent-authored PR receives an independent second-agent review (and at most
one bounded auto-fix + re-review pass when objective `Blocking` defects and/or
clear, low-impact, in-scope `Suggestion / Nit` polish require no human decision)
so PRs reach coffee-break review in a merge-ready state. Simple base-branch sync
merges skip redundant re-reviews once local checks and CI pass on the merge
commit, reducing latency and quota exhaustion while substantive conflict
resolutions remain re-reviewed (and `429` quota failures surface cleanly at the
human gate). Strict scope guardrails, the one-iteration cap, and the human
intervention gate continue to prevent unbounded agent-to-agent loops, scope
creep, or unapproved design changes.
