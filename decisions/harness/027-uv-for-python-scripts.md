# 027: uv for Python script execution

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `mise.lock`, `bin/*.py`
- **Date:** 2026-11-16

## Context

Harness utility scripts in `bin/` (such as `bin/harness.py`) use self-contained
inline script metadata via `uv` so each script runs without a repository-wide
virtualenv or manifest. Without `uv` provisioned inside the devcontainer and CI,
invoking those scripts directly fails. (`host/` scripts run on the host outside
the container and rely on host-side setup.)

## Decision

Pin `uv` in `mise.toml` and lock its platform checksums and attestations in
`mise.lock` so `mise` provisions `uv` on `PATH` in both the devcontainer and
GitHub Actions CI.

## Options Considered

- **Pin `uv` in `mise.toml` and `mise.lock` (chosen):** Unifies `uv` version
  pinning, cross-platform checksum/provenance verification, SBOM tracking, CI
  provisioning, and Renovate dependency updates with the rest of the harness
  toolchain (see [006](006-tool-installation.md) and [015](015-ci-pipeline.md)).
- **Install `uv` via its standalone installer in `setup-user.sh`:** Rejected
  because bypassing `mise` skips `mise.lock` verification and does not carry
  over automatically to CI or Renovate.

## Consequences

Self-contained Python scripts in `bin/` execute directly inside the devcontainer
and CI without a repo-level virtualenv. Scripts without third-party dependencies
avoid outbound network calls; any future script adding third-party dependencies
will need PyPI host allowlist access in sandboxed environments.
