# trivy-plugin-mise

Trivy plugin and standalone CLI (written in Rust) that bridges
[mise](https://github.com/jdx/mise) (`mise.toml`, `mise.lock`, `.mise/locks/`)
with [Trivy](https://github.com/aquasecurity/trivy) for CycloneDX SBOM
generation and vulnerability scanning.

## Architecture & Distribution

- **Implementation language:** Rust (see
  [Decision 001](decisions/001-language-and-distribution.md)).
- **Distribution:** Shipped as a single static `trivy-plugin-mise` binary with a
  root `plugin.yaml` manifest, supporting both
  `trivy plugin install github.com/WJ44/trivy-plugin-mise` (`trivy mise ...`)
  and direct installation via `mise` (`github:WJ44/trivy-plugin-mise`).

## CLI Usage

```bash
cargo run -- --help
cargo run -- --version
cargo run -- sbom --help
cargo run -- scan --help
cargo test
```

<!-- harness:normal-only:start -->

## Development Setup

This repository uses an agentic coding harness designed to run inside a
[Huddle](https://github.com/infosupport/huddle) sandboxed devcontainer with
**Claude Code** (`claude`) or **Antigravity** (`agy`).

### 1. Host: Start the GitHub App token server

Agents authenticate as a GitHub App bot via a host-side token broker mounted at
`/var/run/huddle/agent-token.sock` inside the container.

1. Register a GitHub App and install it on the repository with permissions:
   - `Contents: Read and write`
   - `Issues: Read and write`
   - `Pull requests: Read and write`
   - `Actions: Read-only`
   - `Checks: Read-only`

2. Set `GITHUB_APP_ID` in `host/.agent-identity` (or
   `host/.agent-identity.local`) and start the token server on the host (uses
   [`uv`](https://docs.astral.sh/uv/) via its shebang, or run with Python once
   its inline script dependencies are installed). By default it watches
   `/tmp/dc-sockets` and provisions `agent-token.sock` inside every container
   subdirectory automatically (override with `--sockets-dir` or `--socket`):

   ```bash
   host/agent-token-server.py --key /path/to/app.pem
   ```

### 2. Container: System & user bootstrap

1. **System packages (`gh`, `jq`) — run as `noot`:**

   Enable temporary admin access in the Huddle UI (**Noot** tab), then run:

   ```bash
   su noot
   sudo ./setup.sh --system
   exit
   ```

2. **User tools, bot identity & hooks — run as `vscode`:**

   ```bash
   ./setup.sh --user
   ```

   This installs Antigravity and `mise`, configures `git` and `shim/gh` to act
   as the GitHub App bot, configures `.claude/settings.local.json`,
   `~/.claude.json`, and `~/.gemini/antigravity-cli/settings.json`, installs the
   tools pinned in `mise.toml`, and registers `lefthook` git hooks.

<!-- harness:normal-only:end -->

## Common Commands

### Formatting, Linting & Security

| Command                     | Description                                                 |
| --------------------------- | ----------------------------------------------------------- |
| `lefthook run format`       | Auto-format all files (`--file <path>` for specific files)  |
| `lefthook run format-check` | Check formatting without modifying files                    |
| `lefthook run lint`         | Lint and auto-fix all files where supported                 |
| `lefthook run lint-check`   | Check for lint issues without modifying files               |
| `mise run sbom`             | Generate CycloneDX SBOM (`sbom.cdx.json`)                   |
| `mise run scan`             | Scan dependencies for CVEs and license compliance via Trivy |

<!-- harness:normal-only:start -->

### Harness Management

| Command                                      | Description                                                                        |
| -------------------------------------------- | ---------------------------------------------------------------------------------- |
| `bin/sync-labels [--prune]`                  | Synchronize GitHub issue labels from `.github/labels.json`                         |
| `bin/harness.py init [--release] <dir>`      | Initialize a downstream project (or hydrate an existing release-mode clone)        |
| `bin/harness.py init --in-place [--release]` | Convert an existing template clone in place                                        |
| `bin/harness.py release`                     | Convert an existing project to release mode and purge harness history              |
| `bin/harness.py update [--major]`            | Pull upstream `harness` updates without losing project data                        |
| `bin/harness.py render [--check]`            | Render `AGENTS.md` and `README.md` from templates and `agent-context/*.project.md` |

Project-specific harness inputs (`agent-context/AGENTS.project.md`,
`agent-context/README.project.md`, and `agent-context/memory/MEMORY.md`) live
under `agent-context/`. See
[`docs/harness/harness-cli.md`](docs/harness/harness-cli.md) for full CLI usage,
template rendering, and ownership rules. In **release mode** (`--release` or
`bin/harness.py release`), upstream harness files (`AGENTS.md`, `CLAUDE.md`,
`*.template.md`, `agent-context/harness/`, `decisions/harness/`,
`docs/harness/`, hooks, shims, templates, and setup scripts) remain on disk
locally via a managed `.gitignore` block while project files, `agent-context/`,
`decisions/`, `docs/`, and shared toolchain/CI/release configs (including
`release-please.yml`, `release-please-config.json`,
`.release-please-manifest.json`, and `CHANGELOG.md`) stay tracked in git, and
`README.md` omits harness-only sections.

## Sandbox Allowlist

Common outbound hosts required by baseline harness tools inside Huddle:

- `www.schemastore.org`, `raw.githubusercontent.com` — JSON Schemas fetched by
  `tombi` (TOML formatter/linter) and settings validation
  (`raw.githubusercontent.com` hosts SchemaStore-linked schemas such as
  `rumdl.json`)
- `mirror.gcr.io`, `ghcr.io`, `pkg-containers.githubusercontent.com`,
  `storage.googleapis.com` — Trivy vulnerability database downloads
- `mcp.context7.com` — Context7 remote MCP server for version-specific library
  documentation lookups

## Conventions & Decisions

- **Agent instructions & reference docs:** [`AGENTS.md`](AGENTS.md) and
  [`agent-context/`](agent-context/)
- **Usage & technical documentation:** [`docs/`](docs/) (project-specific) and
  [`docs/harness/`](docs/harness/) (inherited harness baseline, e.g.
  [`harness-cli.md`](docs/harness/harness-cli.md) and
  [`releasing.md`](docs/harness/releasing.md))
- **Decision records:** [`decisions/`](decisions/README.md) (project-specific)
  and [`decisions/harness/`](decisions/harness/README.md) (inherited harness
  baseline)

<!-- harness:normal-only:end -->
