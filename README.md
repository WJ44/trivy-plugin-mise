# trivy-plugin-mise

Project description.

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
   its inline script dependencies are installed):

   ```bash
   host/agent-token-server.py \
     --key /path/to/app.pem \
     --socket /tmp/dc-sockets/<container-name>/agent-token.sock
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
   as the GitHub App bot, configures `.claude/settings.local.json` and
   `~/.gemini/antigravity-cli/settings.json`, installs the tools pinned in
   `mise.toml`, and registers `lefthook` git hooks.

## Common Commands

| Command                     | Description                                                 |
| --------------------------- | ----------------------------------------------------------- |
| `lefthook run format`       | Auto-format all files (`--file <path>` for specific files)  |
| `lefthook run format-check` | Check formatting without modifying files                    |
| `lefthook run lint`         | Lint and auto-fix all files where supported                 |
| `lefthook run lint-check`   | Check for lint issues without modifying files               |
| `mise run sbom`             | Generate CycloneDX SBOM (`sbom.cdx.json`)                   |
| `mise run scan`             | Scan dependencies for CVEs and license compliance via Trivy |
| `bin/sync-labels [--prune]` | Synchronize GitHub issue labels from `.github/labels.json`  |
| `bin/harness.py init <dir>` | Initialize a new downstream project from `harness`          |
| `bin/harness.py update`     | Pull upstream `harness` updates without losing project data |
| `bin/harness.py render`     | Render `AGENTS.md` and `README.md` from templates           |

## Sandbox Allowlist

Common outbound hosts required by baseline harness tools inside Huddle:

- `www.schemastore.org` — JSON Schemas fetched by `tombi` (TOML formatter) and
  settings validation
- `mirror.gcr.io`, `ghcr.io`, `pkg-containers.githubusercontent.com`,
  `storage.googleapis.com` — Trivy vulnerability database downloads

## Conventions & Decisions

- **Agent instructions & reference docs:** [`AGENTS.md`](AGENTS.md) and
  [`agent-context/`](agent-context/)
- **Decision records:** [`decisions/`](decisions/README.md) (project-specific)
  and [`decisions/harness/`](decisions/harness/README.md) (inherited harness
  baseline)
