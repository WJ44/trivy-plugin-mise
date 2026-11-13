# 022: Multi-issue and autonomous task queue orchestration

- **Status:** Active
- **Finality:** Standard (dispatcher pattern; default open-ended task filter is
  Provisional)
- **Scope:** `agent-context/harness/task-queue.md`, `AGENTS.md`
- **Date:** 2026-11-11

## Context

Working through multiple issues or maintenance tasks sequentially inside a
single agent session accumulates context that degrades reasoning quality, wastes
tokens, and risks cross-contaminating git state across branches.

## Decision

When processing multiple tasks or an open-ended backlog, use the main agent
session strictly as a lightweight dispatcher
(`agent-context/harness/task-queue.md`) that resets the workspace to a clean
default branch before each task and delegates each task to a fresh-context
subagent.

## Options Considered

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
- **Parallel execution across git worktrees:** Deferred; sequential execution on
  clean branches avoids concurrent lockfile or container resource contention.

## Consequences

Agents can process multi-issue queues or maintenance backlogs without context
degradation or branch cross-contamination, while still running an independent
clean-context PR review subagent for each opened PR.
