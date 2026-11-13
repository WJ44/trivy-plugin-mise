# 012: Pull request templates for humans and agents

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/PULL_REQUEST_TEMPLATE/`,
  `agent-context/harness/pull-requests.md`
- **Date:** 2026-11-08

## Context

Pull requests are primarily opened by autonomous agents via `gh` CLI and
reviewed by humans who were not in the coding session. Reviewers need a
self-contained hand-off packet to evaluate an agent PR in a few minutes, whereas
occasional human PRs opened in the GitHub web UI only need a lightweight
template.

## Decision

Provide two PR templates in `.github/PULL_REQUEST_TEMPLATE/`:

1. `pull_request_template.md`: Minimal default template auto-loaded by the
   GitHub web UI for human PRs.
2. `agent.md`: Structured hand-off template used by agents via `gh pr create`
   covering context/goal, conceptual summary, reviewer hand-off (trade-offs,
   review focus, caveats/untested items), and task-specific verification.

## Options Considered

- **Dual templates (`pull_request_template.md` for humans + `agent.md` for
  agents) (chosen):** Gives web UI users a lightweight default while giving
  agent PRs a structured review packet.
- **Single detailed PR template for everyone:** Tried initially, then replaced
  because a heavy checklist created friction for quick human PRs in the web UI.

## Consequences

Human PRs in the web UI stay frictionless, while agent PRs provide high-signal
context transfer, surface deferred or untestable scope explicitly, and mirror
issue acceptance criteria without boilerplate noise.
