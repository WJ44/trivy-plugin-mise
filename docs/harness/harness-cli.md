# Harness CLI (`bin/harness.py`)

`bin/harness.py` manages the lifecycle of downstream projects built on
`harness`: initializing new repositories, rendering `AGENTS.md` and `README.md`,
syncing upstream harness updates without clobbering project files, and managing
**release mode**.

Run `bin/harness.py --help` or `bin/harness.py <subcommand> --help` for full
option and flag syntax.

## Ownership Model & `.harness.json`

`bin/harness.py` separates files into clear ownership boundaries:

- **Harness-owned directories and files:** Baseline configuration and workflows
  managed by upstream `harness`—including `agent-context/harness/`,
  `decisions/harness/`, `docs/harness/`, `.github/ISSUE_TEMPLATE/`,
  `.github/PULL_REQUEST_TEMPLATE/`, `.github/workflows/ci.yml`,
  `.github/workflows/release-please.yml`, `hooks/`, `shim/`, `templates/`,
  `bin/`, `.mise/`, `AGENTS.template.md`, `README.template.md`, `CLAUDE.md`,
  `setup*.sh`, and root linter/formatter configs. Mirrored on `init` and
  `update`.
- **Project-owned files:** `agent-context/AGENTS.project.md`,
  `agent-context/README.project.md`, `agent-context/memory/MEMORY.md`,
  project-specific files in `agent-context/`, `decisions/`, and `docs/`,
  `.trivyignore.yaml` (seeded on `init` if missing, then owned by the project),
  `release-please-config.json` and `.release-please-manifest.json` (seeded from
  `templates/` on `init` if missing or overwritten on `init --in-place`, then
  owned by the project), and `CHANGELOG.md` (never hashed in `.harness.json` or
  modified by `update`).
- **Host-only files (`host/`):** Present only in the upstream `harness`
  repository; never copied downstream (and removed by `init --in-place`).

Sync state is recorded in `.harness.json` (`upstream`, `ref`, `commit`,
`version`, `mode`, and SHA-256 hashes of harness-owned files) so `update` can
detect local edits and major-version boundaries before modifying files.

## Template Rendering (`render`)

`AGENTS.md` and `README.md` are generated from paired template and project input
files:

- `AGENTS.template.md` + `agent-context/AGENTS.project.md` -> `AGENTS.md`
- `README.template.md` + `agent-context/README.project.md` -> `README.md`

Named blocks wrapped in `<!-- project:<name>:start -->` /
`<!-- project:<name>:end -->` inside `agent-context/*.project.md` are injected
into matching `{{project:<name>}}` placeholders in the template. In `release`
mode, `<!-- harness:normal-only:start -->` / `<!-- harness:normal-only:end -->`
sections are stripped from the rendered output.

```bash
bin/harness.py render          # Regenerate AGENTS.md and README.md
bin/harness.py render --check  # Exit non-zero if rendered files are out of date
```

## Commands

### `init` — Initialize or hydrate a project

When `--ref` is omitted, `init` resolves the latest `vX.Y.Z` release tag from
upstream (falling back to `main` if no `vX.Y.Z` tags exist). Pass
`--ref <branch|tag>` to target a specific tag or branch explicitly.

- **Create a new downstream project:**

  ```bash
  bin/harness.py init <target-dir> [--name "<Title>"] [--description "<Summary>"] [--ref <branch|tag>] [--release]
  ```

  Copies harness-owned files, seeds clean `decisions/README.md`,
  `agent-context/*.project.md`, `release-please-config.json`, and
  `.release-please-manifest.json` starters (preserving existing release config
  or manifest files if already present), writes `.harness.json` (including the
  resolved `"version"`), renders `AGENTS.md` and `README.md`, and initializes
  git (`main`) if needed.

- **Convert a GitHub template clone in place:**

  ```bash
  bin/harness.py init --in-place [--release]
  ```

  Removes `host/`, `harness`'s own `tests/` (when `tests/run.py` is present),
  `harness`'s own `decisions/*.md` records, upstream `CHANGELOG.md`,
  `agent-context/releasing.md`, and `docs/releasing.md`, resets project starters
  (including `release-please-config.json` and `.release-please-manifest.json`
  from `templates/`), writes `.harness.json`, and renders `AGENTS.md` and
  `README.md`.

- **Hydrate an existing release-mode clone:**

  Running `bin/harness.py init <clone-dir>` on an existing git clone that uses
  release mode restores gitignored local harness files
  (`agent-context/harness/`, `decisions/harness/`, `docs/harness/`, hooks,
  shims, templates, and `bin/harness.py`) without modifying tracked project
  files, `README.md`, or git history.

### `update` — Pull upstream harness updates

```bash
bin/harness.py update [--dry-run] [--source <path|url>] [--ref <branch|tag>] [--major] [--force]
```

Resolves the latest upstream `vX.Y.Z` release tag by default when `--ref` is
omitted (falling back to `main` if no `vX.Y.Z` tags exist), previews or applies
changes to harness-owned paths, updates `.harness.json`, and re-renders
`AGENTS.md` and `README.md`.

- Requires a valid SemVer `"version"` in `.harness.json` and a clean git working
  tree (unless `--dry-run` or `--force` is passed).
- Compares installed `.harness.json` `"version"` against the target upstream
  `HARNESS_VERSION`:
  - **Patch/minor upgrades** proceed directly (printing relevant release notes
    from upstream `CHANGELOG.md` when available).
  - **Major upgrades (`target.major > current.major`)** extract and print
    release and breaking-change notes for `(current, target]` from upstream
    `CHANGELOG.md` and block with a non-zero exit code unless `--major` or
    `--force` is passed (`--dry-run` previews the notes and warning alongside
    planned file changes).
- Refuses to overwrite or delete files that are locally modified or untracked in
  `.harness.json` unless `--force` is specified.
- In `release` mode, preserves project-added scripts in `bin/` and `.mise/` not
  recorded in `.harness.json`, and only updates the managed `.gitignore` block.

### `release` — Convert to release mode

**Release mode** keeps upstream agent/harness scaffolding (`AGENTS.md`,
`CLAUDE.md`, `*.template.md`, `agent-context/harness/`, `decisions/harness/`,
`docs/harness/`, `hooks/`, `shim/`, `templates/`, `setup*.sh`, `bin/harness.py`,
`bin/sync-labels`, `bin/wait-pr-checks`, `.harness.json`, `.mcp.json`, and
`.github/` templates/labels) local and gitignored while keeping project code,
`agent-context/`, `decisions/`, `docs/`, and shared toolchain/CI/release configs
(`mise.toml`, `lefthook.yml`, `.rumdl.toml`, `bin/mise-sbom`,
`bin/validate-decisions.py`, `bin/validate-trivyignore.yq`,
`.github/workflows/ci.yml`, `.github/workflows/release-please.yml`,
`release-please-config.json`, `.release-please-manifest.json`, and
`CHANGELOG.md`) tracked in git.

```bash
bin/harness.py release
```

Converts an existing normal-mode repository into release mode by:

1. Rewriting git history with `git-filter-repo` to purge excluded harness paths
   and `normal-only` sections in `README.md` and `decisions/README.md` (requires
   `git-filter-repo` via `mise install`).
2. Restoring the excluded harness files locally on disk and adding the managed
   `# --- harness:release:start ---` / `# --- harness:release:end ---` block to
   `.gitignore`.
3. Recording `"mode": "release"` in `.harness.json`, re-rendering `AGENTS.md`
   and `README.md`, stripping `normal-only` sections from `decisions/README.md`,
   and committing the tracked `.gitignore`, `README.md`, and
   `decisions/README.md` updates.

Converting to release mode is **one-way** and rewrites git history.
