# 018: Agent lifecycle hooks for post-edit formatting and linting

- **Status:** Active
- **Finality:** Standard
- **Scope:** `hooks/post-edit`, `.claude/settings.json`, `.agents/hooks.json`
- **Date:** 2026-11-09

## Context

When agents edit files, waiting until `git commit` to run formatters and linters
delays feedback on lint errors. Both Claude Code and Antigravity support
post-edit lifecycle hooks, though each uses a different hook configuration file
and JSON input/output protocol.

## Decision

Configure post-edit lifecycle hooks in `.claude/settings.json` and
`.agents/hooks.json` that invoke a single shared script (`hooks/post-edit`) to
auto-format edited repository files via `lefthook` and feed any unfixable lint
errors back to the calling agent.

## Options Considered

- **Single shared script in `hooks/post-edit` (chosen):** Keeps vendor hook
  configs minimal and ensures both agent harnesses run the exact same `lefthook`
  formatting and linting pipeline.
- **Separate harness-specific hook scripts:** Rejected to avoid duplicating
  path-filtering and `lefthook` invocation logic per agent harness.
- **Inline shell commands in JSON config:** Rejected because multi-step error
  handling in JSON strings is fragile and cannot be linted by `shfmt` or
  `shellcheck`.

## Consequences

Files edited by agents are formatted immediately, and unfixable lint errors are
reported back on the next turn so the agent can fix them right away.
