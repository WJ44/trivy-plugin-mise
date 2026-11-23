# 018: Agent lifecycle hooks for post-edit formatting and linting

- **Status:** Active
- **Finality:** Standard
- **Scope:** `hooks/post-edit`, `.claude/settings.json`, `.agents/hooks.json`
- **Date:** 2026-11-18

## Context

When agents edit files, waiting until `git commit` to run formatters and linters
delays feedback on lint errors. Both Claude Code and Antigravity support
post-edit lifecycle hooks, though each uses a different hook configuration file
and JSON input/output protocol.

However, when an edited file still has unresolved git merge conflicts, running
formatters or linters after a partial edit corrupts remaining conflict markers
(for example, Prettier reinterpreting closing markers as nested Markdown
blockquotes) or produces noisy syntax errors.

## Decision

Configure post-edit lifecycle hooks in `.claude/settings.json` and
`.agents/hooks.json` that invoke a single shared script (`hooks/post-edit`) to
auto-format edited repository files via `lefthook` and feed any unfixable lint
errors back to the calling agent. Skip formatting and linting whenever the
edited file has unmerged git index entries or unresolved conflict markers.

## Options Considered

- **Single shared script in `hooks/post-edit` (chosen):** Keeps vendor hook
  configs minimal and ensures both agent harnesses run the exact same `lefthook`
  formatting and linting pipeline.
- **Separate harness-specific hook scripts:** Rejected to avoid duplicating
  path-filtering and `lefthook` invocation logic per agent harness.
- **Inline shell commands in JSON config:** Rejected because multi-step error
  handling in JSON strings is fragile and cannot be linted by `shfmt` or
  `shellcheck`.
- **Skip on unmerged git index entries or conflict markers (chosen):** Prevents
  formatter corruption and noisy mid-conflict lint errors while also catching
  conflict markers in prematurely staged or manually edited files.
- **Skip on unmerged git index entries only:** Rejected because it misses
  conflict markers when a file is staged prematurely or edited manually with
  markers present.

## Consequences

Files edited by agents are formatted immediately when free of unmerged git index
entries and conflict markers, and unfixable lint errors are reported back on the
next turn so the agent can fix them right away. Files with unresolved merge
conflicts remain untouched by post-edit hooks until resolved in both the working
tree and git index, while git still blocks commits with unmerged index entries
and `pre-commit` syntax/lint checks continue to run on staged code and config
files.
