# 020: Subagent delegation and model tier selection for bulk reads

- **Status:** Active
- **Finality:** Standard
- **Scope:** `AGENTS.template.md`, `AGENTS.md`
- **Date:** 2026-11-18

## Context

Large or numerous file reads clutter the main agent's context window and consume
significant tokens when the primary session runs on a large model. In addition,
some agent CLIs default subagents to inheriting the parent session's model
unless explicitly instructed otherwise, while Antigravity exposes concrete model
names in `/model` and `agy models` but routes subagents through abstract tier
enums. At the same time, delegating to an asynchronous subagent incurs
background wakeup round-trip latency when a quick targeted search would locate
the needed files immediately. Because file-reading and search decisions happen
continuously throughout every session, guidance on when to delegate must be
present in `AGENTS.md` from the start of the session.

## Decision

Include a concise `Subagent delegation` rule in `AGENTS.md` (rendered from
`AGENTS.template.md`): handle targeted lookups (reading 1–3 known files or
`rg`/`grep` search queries with small output) directly in the main session,
delegate broad exploration or bulk reads (>3–4 files or large logs) to a
read-only subagent on a smaller, faster model tier, and have the subagent return
synthesized findings rather than raw file contents.

In Antigravity, `invoke_subagent`'s `Model` parameter accepts abstract tier
enums (`"flash"`, `"flash_lite"`, `"pro"`, `"inherit"`) that automatically route
to the active model for that tier (e.g., Gemini 4 Hydrogen for `"flash"`) rather
than concrete model IDs from `agy models`. `AGENTS.md` therefore specifies tier
names (`"flash"`, `"flash_lite"`) rather than pinning concrete model names.

## Options Considered

- **Rule in `AGENTS.md` using built-in read-only subagents (chosen):** Always in
  active context across both Antigravity and Claude Code, and overrides default
  model inheritance for read-only exploration without affecting complex
  subagents.
- **Keep targeted `rg`/`grep` searches in the main session alongside 1–3 known
  files (chosen):** Avoids asynchronous subagent round-trip latency when 1–2
  targeted searches with small output suffice to locate files.
- **Document Antigravity tier enum routing in this decision record (chosen):**
  Records why `AGENTS.md` specifies `"flash"` and `"flash_lite"` without adding
  Antigravity-specific model mapping prose to `AGENTS.md`.
- **Explain model tier resolution directly inside `AGENTS.template.md`:**
  Rejected to keep `AGENTS.md` compact since it is loaded into every session's
  context in full.
- **Separate `agent-context/subagents.md` file:** Rejected because file-reading
  decisions happen on every turn before an agent would consult an external
  reference file.
- **Global subagent model environment variable (`CLAUDE_CODE_SUBAGENT_MODEL`):**
  Rejected because it forces all subagents—including implementation and PR
  review subagents—onto a smaller model.

## Consequences

Main agent sessions preserve context window capacity and token budget for
planning, reasoning, and editing without incurring asynchronous subagent latency
on quick targeted lookups, while read-only exploration runs on fast,
cost-effective model tiers that adapt automatically as active tier models change
(e.g., Gemini 4 Hydrogen for `"flash"`).
