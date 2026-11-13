# 019: Agent PR review process

- **Status:** Active
- **Finality:** Standard
- **Scope:** `agent-context/harness/pull-requests.md`, `AGENTS.md`
- **Date:** 2026-11-09

## Context

Pull requests are primarily authored by autonomous agents and reviewed by humans
during short review windows. Having the authoring agent self-review within its
own session risks confirmation bias from accumulated reasoning, while requiring
a human to manually spin up a second session before reviewing adds friction.

## Decision

After opening a PR, the authoring agent spawns a subagent with a clean context
(passing only the PR number and review instructions) to perform an independent,
report-only review and post a structured review comment on the PR, after which
both agents stop and wait for human direction.

## Options Considered

- **Clean-context subagent + `agent-context/harness/pull-requests.md` rubric
  (chosen):** Works natively across supported agent CLIs, isolates the reviewer
  from the author's reasoning history, and triggers automatically when a PR is
  opened.
- **Portable skill (`.agents/skills/` + `.claude/skills/`):** Rejected because
  PR review is already part of the standard PR workflow in
  `agent-context/harness/pull-requests.md`.
- **Wrapper script (`bin/review-pr`):** Rejected because it requires hardcoding
  or detecting which agent CLI and model to invoke headlessly.
- **Manual review trigger only:** Rejected because it requires a human to
  manually launch a second session for every PR.

## Consequences

Every agent-authored PR receives an independent second-agent review comment
before human review, and the human intervention gate prevents unbounded
agent-to-agent fix loops without human oversight.
