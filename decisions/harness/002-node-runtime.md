# 002: Node.js as JavaScript runtime for tooling

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml` (`node` and `npm:` tool backends)
- **Date:** 2026-11-03

## Context

JavaScript-based CLI tools such as Prettier and `commitlint` require a
JavaScript runtime. Bun was considered as a single fast runtime, but npm CLI
binaries ship with a `#!/usr/bin/env node` shebang and still require `node` on
`PATH`.

## Decision

Use Node.js as the JavaScript runtime for repository tooling.

## Options Considered

- **Node.js (chosen):** Directly satisfies `#!/usr/bin/env node` shebangs used
  by npm CLI binaries without shims or workarounds.
- **Bun:** Evaluated for speed and unified tooling, but rejected because npm bin
  launchers still invoke `node` directly.

## Consequences

Every `npm:` tool in `mise.toml` works out of the box. Revisit if `mise` and npm
bin launchers natively support running under Bun without a `node` binary.
