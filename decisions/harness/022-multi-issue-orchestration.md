# 022: Multi-issue and autonomous task queue orchestration

- **Status:** Active
- **Finality:** Standard (dispatcher pattern and worktree isolation; default
  open-ended task filter is Provisional)
- **Scope:** `agent-context/harness/task-queue.md`,
  `agent-context/harness/pull-requests.md`, `agent-context/harness/issues.md`,
  `AGENTS.md`
- **Date:** 2026-11-23

## Context

Working through multiple issues or maintenance tasks sequentially inside a
single agent session accumulates context that degrades reasoning quality, wastes
tokens, and risks cross-contaminating git state across branches. Furthermore,
because autonomous agent PRs stop at the human intervention gate after review,
dependent issues cannot advance against `main` alone while prerequisite PRs
remain unmerged.

Initial orchestration relied exclusively on `Workspace: "inherit"`, requiring
strict workspace exclusivity and serialization across all task, sync, and
clean-context PR review subagents. Both Antigravity (`Workspace: "share"`) and
Claude Code (`isolation: "worktree"`) now support isolated git worktrees sharing
the repository's `.git` object store, prompting an empirical evaluation inside
the Huddle container (`8 GiB` memory limit, `2.0` vCPU quota) across `mise`,
`lefthook`, `hooks/post-edit`, agent trust/local settings, release-mode
hydration, and concurrent resource load.

## Decision

When processing multiple tasks or an open-ended backlog, use the main agent
session strictly as a lightweight dispatcher
(`agent-context/harness/task-queue.md`) that delegates each task to a
fresh-context subagent. Reset the workspace to a clean default branch before
independent tasks, and stack single-parent dependent tasks linearly onto a
qualifying unmerged parent PR branch (open, passing verification and CI, and
clean-context reviewed without `Blocking` or `Needs Human Decision` findings)
across both open-ended and explicit list modes. Multi-parent (diamond)
dependencies and dependents of failed or blocked parent PRs are skipped until
resolved.

For subagent workspace isolation, use a mode-aware policy:

- **Normal mode (`share`/worktree with bounded parallelism):** When harness
  files are tracked in git, run task, sibling-sync, and clean-context PR review
  subagents in isolated git worktrees (`Workspace: "share"` in Antigravity;
  `isolation: "worktree"` in Claude Code) so active subagents never collide with
  the dispatcher's checkout. Independent, non-overlapping tasks and asynchronous
  PR reviews may run with bounded parallelism (2–3 concurrent subagents), while
  dependent chains and heavy resource scans remain serialized.
- **Release mode (`inherit` fallback):** When harness files are gitignored via
  release mode (`RELEASE_EXCLUDED_PATHS`) and not hydrated into fresh worktrees,
  fall back to `Workspace: "inherit"` with strict serial workspace exclusivity.

## Options Considered

- **Mode-aware `share`/worktree isolation with bounded parallelism (chosen):**
  Empirical sandbox testing showed Antigravity `Workspace: "share"` creates git
  worktrees where `mise` auto-trusts `mise.toml` for worktrees of a trusted
  repo, shared `.git/hooks` (`pre-commit`, `commit-msg`) and tracked
  `.agents/hooks.json` + `hooks/post-edit` fire normally, and two concurrent
  `lefthook` format/lint runs complete in ~1.1s each within container limits.
  Eliminates `inherit` workspace-exclusivity collisions and enables bounded
  parallelism for independent tasks and asynchronous reviews, while falling back
  to `inherit` in release mode where gitignored harness files are absent.
- **Reviewers-only `share`/worktree:** Rejected for normal mode because keeping
  implementation and sync subagents on `inherit` leaves dispatcher-versus-author
  workspace-exclusivity fragility and blocks safe concurrency across unrelated
  issues and sibling PR `main`-sync passes, while saving little setup overhead
  given `mise` and `lefthook` already work in worktrees.
- **Retain `Workspace: "inherit"` everywhere:** Rejected because `inherit`
  forces strict serialization even for read-only reviews and independent tasks
  despite normal-mode worktrees requiring no `setup-user.sh` changes.
- **Dispatcher session + fresh-context subagent per task (chosen):** Works
  natively across supported agent CLIs, guarantees a clean context window and
  clean git branch state per task, and avoids CLI-specific shell wrapper
  scripts.
- **External shell loop (`bin/agent-loop`):** Rejected because it requires
  hardcoding CLI-specific headless flags and loses interactive visibility in the
  active session.
- **Portable skill (`.agents/skills/` + `.claude/skills/`):** Rejected because a
  reference doc in `agent-context/harness/task-queue.md` linked from `AGENTS.md`
  avoids extra skill directories while staying out of single-issue session
  context.
- **Single-parent linear stacked PRs via native `git`/`gh` (chosen for
  dependencies):** Preserves small, incremental child diffs and lets linear
  chains progress past the human intervention gate without external stacking
  wrappers or custom auto-rebase actions.
- **Skip all dependent or `blocked`-labeled issues until parent PRs merge:**
  Rejected as overly restrictive for linear dependency chains during multi-issue
  runs.
- **Multi-parent integration branches or automated rebase/merge workflows:**
  Deferred/rejected to avoid merge-conflict complexity, custom wrapper tooling,
  and bypassing the human intervention gate.

## Consequences

Agents can process multi-issue queues—including linear dependency chains and
bounded parallel independent tasks or PR reviews in normal mode—without context
degradation or branch cross-contamination, while preserving incremental diffs,
independent clean-context PR reviews, and the human intervention gate on every
PR. Known boundaries from sandbox testing apply: gitignored
`.claude/settings.local.json` (`autoMemoryDirectory`) and per-path trust entries
(`~/.claude.json`, Antigravity `trustedWorkspaces`) do not follow dynamic
worktree paths (Claude Code worktrees fall back from project memory redirection;
Antigravity `toolPermission: "always-proceed"` and tracked `AGENTS.md` still
applied), release-mode clones stay on `inherit` unless worktree hydration is
added later, and finished subagents must be killed (`manage_subagents`
`Action: "kill"`) so their temporary worktrees and branches are pruned before
local branch cleanup. Dependent tasks with multiple unmerged parents or unmet
stacking preconditions wait cleanly for human intervention.
