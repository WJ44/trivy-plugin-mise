# 025: Unattended agent execution inside the sandbox

- **Status:** Active
- **Finality:** Locked
- **Scope:** `.claude/settings.json`, `setup-user.sh`,
  `agent-context/harness/agent-configuration.md`,
  `agent-context/harness/sandbox.md`
- **Date:** 2026-11-13

## Context

Agents using this harness are intended to work autonomously—including during
unsupervised and multi-issue sessions—without blocking on interactive
confirmation prompts. Because sessions run inside a sandboxed container with
network allowlisting and long-lived credentials kept on the host, in-tool
permission prompts and file-read deny rules add friction without forming a real
security boundary (agents already have shell access inside the container).

## Decision

Configure supported agent CLIs to run unattended without interactive tool
approval prompts (`bypassPermissions` in Claude Code, `always-proceed` in
Antigravity), relying on the container sandbox, outbound network allowlist, and
host-side credential isolation as the security boundary.

## Options Considered

- **Unattended execution backed by the container sandbox (chosen):** Allows
  uninterrupted autonomous execution across all tools while keeping the real
  security boundary at the container, network proxy, and host-side token broker.
- **Interactive / classifier-gated prompts (`auto` mode) and tool-level
  read-deny rules:** Rejected because interactive prompts stall autonomous
  sessions and tool-level file-read rules are ineffective when shell execution
  is enabled inside the container.

## Consequences

Agents execute tools without confirmation prompts. Long-lived secrets (such as
GitHub App private keys) must remain outside the container on the host, and
outbound network access is governed by the sandbox allowlist rather than
in-agent permission prompts.
