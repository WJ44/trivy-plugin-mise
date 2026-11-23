# Testing harness scripts

Automated tests for the upstream `harness` scripts (`bin/`, `hooks/`, `host/`,
and `setup-user.sh`) live in [`tests/`](../../tests/) and run with `pytest` via
[`uv`](../../decisions/harness/027-uv-for-python-scripts.md).

## When to add or update tests

- **Add or update tests when:**
  - Adding new functionality, flags, subcommands, hooks, or lifecycle flows to
    harness scripts (`bin/`, `hooks/`, `host/`, `setup-user.sh` — and
    analogously to project code in downstream repos with a test suite).
  - Fixing a bug or regression (include a focused regression test that would
    have failed before the fix).
  - Refactoring nontrivial stateful, concurrent, parsing, or git/filesystem
    logic.
  - Changing template rendering, release-mode hydration, or
    `init`/`update`/`release` ownership behavior.
- **When tests are generally not required:**
  - Pure documentation (`docs/`, `agent-context/`, `decisions/`, `README*`) or
    static config edits that do not change executable script logic (existing
    checks such as `bin/harness.py render --check` and
    `bin/validate-decisions.py` cover rendered and validated invariants).
  - External boundaries explicitly out of scope (live GitHub APIs,
    Docker-in-Docker mounts); stub or isolate boundaries in temporary
    directories instead, and note any untestable sandbox gaps in the PR
    `Reviewer Hand-off`.
- **Before opening or updating a PR:** Run `mise run test` whenever executable
  scripts or tests are modified.

## Running tests locally

Run the full suite through `mise` (which places pinned `lefthook`, `ruff`,
`rumdl`, `shellcheck`, `shfmt`, `tombi`, `yq`, `uv`, and `git-filter-repo` on
`PATH`):

```bash
mise run test
```

Pass `pytest` flags or explicit test paths/nodeids after `--` (for example, to
filter, increase verbosity, or target a single test file):

```bash
mise run test -- -k test_release -v
mise run test -- tests/test_harness.py
```

You can also invoke the PEP 723 runner directly (ensuring `mise` shims are on
`PATH` for CLI/hook subprocess tests):

```bash
mise x -- uv run --script tests/run.py
```

For individual test files that use only the Python standard library (all tests
except
[`tests/test_agent_token_server.py`](../../tests/test_agent_token_server.py)):

```bash
mise x -- uv run --with pytest pytest tests/test_harness.py
```

## What is covered

- `host/agent-token-server.py`: directory/socket preflight checks, stale socket
  probing and cleanup, `0666` socket permissions under `umask 0o111`, inode
  ownership checks, `SocketManager` reconciliation and `MultiSocketServer`
  shutdown cleanup, and `/token` endpoint caching and thread safety with
  `PyGithub` stubbed.
- `bin/harness.py`: template rendering and `normal-only` stripping, SemVer and
  changelog extraction, `init`, `init --in-place`, `update` (conflict checks,
  major-version guards, dry runs), `release` history rewriting, and release-mode
  clean-checkout `format-check` / `lint-check` smoke tests.
- `bin/validate-decisions.py`: filename, H1, metadata order/values,
  `Superseded by` links, required H2 sections, and `README.md` index sync.
- `hooks/post-edit`: Claude Code and Antigravity payloads, out-of-repo/missing
  file ignores, formatting, error reporting, and merge-conflict / unmerged-index
  skipping.
- `setup-user.sh`: user guards, `--help`, isolated `HOME` workspace trust
  merging (`~/.claude.json`, `~/.gemini/antigravity-cli/settings.json`), local
  Claude memory config, shell `PATH` idempotency, git config, and attestation
  failure handling with stubbed network/socket boundaries.

Live GitHub API calls and Docker-in-Docker Huddle mounts are intentionally out
of scope; external APIs and system boundaries are stubbed or isolated in
temporary directories.

## Downstream scoping (`init`, `update`, and `release`)

`tests/` is reserved for project-specific tests in downstream repositories:

- **`init <dir>` and `update`:** `tests/` is not in `HARNESS_OWNED_DIRS` or
  `HARNESS_OWNED_FILES`, so upstream harness tests are never copied or synced
  into downstream repositories.
- **`init --in-place`:** When converting a template clone of `WJ44/harness`,
  `bin/harness.py init --in-place` deletes `tests/` if `tests/run.py` is present
  (alongside `host/` and `decisions/*.md`) so the downstream project starts with
  a clean slate.
- **`release`:** `tests/` is not in `RELEASE_EXCLUDED_PATHS`, so downstream
  application test suites under `tests/` stay tracked in git in release mode.
- **CI and `mise.toml`:** `.github/workflows/ci.yml` (`Harness tests` job) and
  `mise.toml` (`[tasks.test]`) check for `tests/run.py` before invoking the
  harness runner, so downstream CI stays green out of the box. Note that
  `mise.toml` is in `HARNESS_OWNED_FILES`, so customizing `[tasks.test]`
  directly in `mise.toml` (rather than adding a separate project config or CI
  job) will trigger a sync conflict on `bin/harness.py update` until resolved.
