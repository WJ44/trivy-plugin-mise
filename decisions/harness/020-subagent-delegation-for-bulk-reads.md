# 020: Subagent delegation and model tier selection for bulk reads

- **Status:** Active
- **Finality:** Standard
- **Scope:** `AGENTS.md`
- **Date:** 2026-11-09

## Context

Large or numerous file reads clutter the main agent's context window and consume
significant tokens when the primary session runs on a large model. In addition,
some agent CLIs default subagents to inheriting the parent session's model
unless explicitly instructed otherwise. Because file-reading happens
continuously throughout every session, guidance on when to delegate must be
present in `AGENTS.md` from the start of the session.

## Decision

Include a concise `Subagent delegation` rule in `AGENTS.md`: read 1–3 known
files directly in the main session, delegate broad exploration or bulk reads
(>3–4 files or large logs) to a read-only subagent on a smaller, faster model
tier, and have the subagent return synthesized findings rather than raw file
contents.

## Options Considered

- **Rule in `AGENTS.md` using built-in read-only subagents (chosen):** Always in
  active context across both Antigravity and Claude Code, and overrides default
  model inheritance for read-only exploration without affecting complex
  subagents.
- **Separate `agent-context/subagents.md` file:** Rejected because file-reading
  decisions happen on every turn before an agent would consult an external
  reference file.
- **Global subagent model environment variable (`CLAUDE_CODE_SUBAGENT_MODEL`):**
  Rejected because it forces all subagents—including implementation and PR
  review subagents—onto a smaller model.

## Consequences

Main agent sessions preserve context window capacity and token budget for
planning, reasoning, and editing, while read-only exploration runs on fast,
cost-effective model tiers.
