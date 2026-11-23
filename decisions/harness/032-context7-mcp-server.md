# 032: Context7 remote HTTP MCP server

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.mcp.json`, `.claude/settings.json`, `.agents/plugins/context7/`,
  `bin/harness.py`, `setup-user.sh`, `AGENTS.template.md`, `README.template.md`,
  `agent-context/harness/agent-configuration.md`,
  `agent-context/harness/sandbox.md`
- **Date:** 2026-11-18

## Context

When working with external libraries and frameworks, coding agents relying on
model training data or general web search can surface outdated APIs or miss
version-specific documentation. [Context7](https://context7.com) exposes
version-specific library documentation and code snippets via the Model Context
Protocol (`resolve-library-id` and `query-docs`) over a hosted Streamable HTTP
endpoint (`https://mcp.context7.com/mcp`).

The baseline harness prefers project-scoped, unattended configuration
([025](025-unattended-agent-execution-in-sandbox.md)), aims to keep the pinned
toolchain in `mise.lock` and its SBOM/Trivy surface minimal
([021](021-software-bill-of-materials-and-vulnerability-scanning.md),
[023](023-dev-toolchain-sbom-and-scanning.md)), and syncs baseline agent files
to downstream projects via `bin/harness.py` (excluding upstream agent
scaffolding in release mode).

## Decision

Configure the unauthenticated remote Context7 Streamable HTTP MCP server at the
project level for both supported agent CLIs:

- **Claude Code:** Define `context7` in `.mcp.json`, pre-approve it in
  `.claude/settings.json` via `enabledMcpjsonServers` rather than blanket
  project-wide MCP approval, and register workspace trust in `~/.claude.json`
  via `setup-user.sh` so unattended sessions honor project approvals.
- **Antigravity (`agy`):** Package the `context7` server inside a workspace
  plugin under `.agents/plugins/context7/`.
- **Lifecycle, agent guidance & sandbox:** Sync these files as harness-owned
  files in `bin/harness.py` (excluded from git in release mode alongside
  `.claude/` and `.agents/`), document `mcp.context7.com` in the sandbox
  allowlist, and instruct agents in `AGENTS.template.md` and
  `agent-context/harness/sandbox.md` to prefer Context7 (`resolve-library-id`
  then `query-docs`) for version-specific library/tool docs before falling back
  to general web search.

## Options Considered

- **Remote Streamable HTTP MCP server, unauthenticated (chosen):** Provides
  native, structured, version-specific documentation tools in both Claude Code
  and Antigravity without adding npm packages to `mise.toml` / `mise.lock` or
  requiring API-key brokering in `host/agent-token-server.py`. Requires separate
  vendor-specific MCP config files because Claude Code and Antigravity use
  incompatible project-level MCP schemas and discovery paths, and requires
  `mcp.context7.com` on the Huddle outbound allowlist.
- **Local stdio MCP (`@upstash/context7-mcp`) or `ctx7` CLI in `mise.toml`:**
  Rejected because bundling local Node packages expands `mise.lock`, the
  CycloneDX SBOM, and ongoing Trivy vulnerability/license maintenance without
  offering meaningful advantages over the hosted Streamable HTTP endpoint.
- **Built-in web search only:** Rejected as the sole documentation mechanism
  because general web searches often mix library versions and lack structured
  library-ID resolution and focused snippet queries; retained as the fallback
  for general web lookups.

## Consequences

- Both Claude Code and Antigravity expose Context7 MCP documentation tools
  automatically once `mcp.context7.com` is permitted on the Huddle sandbox
  allowlist and `setup-user.sh` has registered workspace trust (`~/.claude.json`
  for Claude Code; `~/.gemini/antigravity-cli/settings.json` for Antigravity).
- No npm packages or `CONTEXT7_API_KEY` secret handling are added to `mise.lock`
  or `host/agent-token-server.py`, and Claude Code auto-approval is scoped
  solely to `context7` rather than all future `.mcp.json` servers.
- Downstream projects inherit and update the Context7 config files through
  `bin/harness.py` (retained locally and gitignored in release mode).
- Revisit if unauthenticated Context7 rate limits require `CONTEXT7_API_KEY`
  brokering, if Claude Code and Antigravity adopt a shared MCP configuration
  format, or if the hosted endpoint changes or is retired.
