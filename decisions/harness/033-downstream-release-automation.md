# 033: Downstream release automation

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/workflows/release-please.yml`,
  `templates/release-please-config.json`,
  `templates/release-please-manifest.json`, `bin/harness.py`,
  `agent-context/harness/releasing.md`, `docs/harness/releasing.md`
- **Date:** 2026-11-23

## Context

Downstream projects initialized from `harness` previously inherited CI,
formatting, linting, and Trivy/SBOM scanning, but
`.github/workflows/release-please.yml`, `release-please-config.json`,
`.release-please-manifest.json`, and `CHANGELOG.md` were upstream-only and
excluded in `release` mode. Every downstream project therefore had to recreate
SemVer release automation from scratch, while upstream `harness` started at
`1.0.0` with a `bin/harness.py` updater specific to the harness repository
itself.

## Decision

Share `release-please` release automation with downstream projects using the
same ownership split as CI and Trivy, kept tracked in git in both `normal` and
`release` modes:

1. **Harness-owned workflow with a safe skip guard:** Manage
   `.github/workflows/release-please.yml` in `HARNESS_OWNED_FILES` and add a
   pinned checkout and prerequisite check before
   `googleapis/release-please-action`. When `release-please-config.json`,
   `.release-please-manifest.json`, or `secrets.RELEASE_PLEASE_TOKEN` is
   missing, emit a `::notice::` and skip `release-please` without failing `main`
   CI.
2. **Template-seeded, project-owned configuration and state:** Seed
   `release-please-config.json` (`simple` strategy, pre-1.0 bump rules, no
   upstream `bin/harness.py` updater) and `.release-please-manifest.json`
   (`0.0.0`) from `templates/` on `init` when absent (and reset them alongside
   removing upstream `CHANGELOG.md`, `agent-context/releasing.md`, and
   `docs/releasing.md` on `init --in-place`), without hashing them in
   `.harness.json` or modifying them during `update`.
3. **Tracked in `release` mode:** Exclude
   `.github/workflows/release-please.yml`, `release-please-config.json`,
   `.release-please-manifest.json`, and `CHANGELOG.md` from
   `RELEASE_EXCLUDED_PATHS` so downstream release metadata and changelogs stay
   tracked in git.
4. **Separated baseline and upstream docs:** Keep generic `release-please` rules
   and setup guides in `agent-context/harness/releasing.md` and
   `docs/harness/releasing.md`, while keeping `agent-context/releasing.md` and
   `docs/releasing.md` focused on `WJ44/harness`-specific policies.

## Options Considered

- **Harness-owned workflow with skip guard + template-seeded project config
  (chosen):** Downstream projects receive workflow updates (including pinned
  action SHAs) via `bin/harness.py update`, can customize `release-type` or
  `extra-files` without sync conflicts, start safely at `0.0.0` until promoted
  to `1.0.0`, and never break `main` CI before `RELEASE_PLEASE_TOKEN` is added
  or on pre-existing repos that have not adopted `release-please`.
- **Fallback to `GITHUB_TOKEN` when `RELEASE_PLEASE_TOKEN` is unset:** Rejected
  because `GITHUB_TOKEN` cannot open Release PRs under default repository
  permissions and suppresses `on: pull_request` CI and tag/release triggers on
  downstream artifact workflows (see
  [002](../002-semantic-versioning-and-releases.md)).
- **Auto-seed `release-please-config.json` and `.release-please-manifest.json`
  during `bin/harness.py update`:** Rejected so existing downstream repos that
  manage releases differently are not opted in unexpectedly; they opt in
  explicitly when ready, while the workflow guard keeps pushes green.
- **Bundle language-specific artifact build/publish workflows in `harness`:**
  Rejected because packaging, checksums, provenance, and registry publishing
  vary by stack and belong in project-specific workflows triggered by `v*` tags
  or `release: [published]`.

## Consequences

- Newly initialized downstream projects have ready-to-use `release-please`
  scaffolding starting at `0.0.0` (where `fix:`/`feat:` bump patch and breaking
  changes bump minor until promoted to `1.0.0`) and stay green on `main` until a
  maintainer configures `RELEASE_PLEASE_TOKEN`.
- Existing downstream repos upgrading via `bin/harness.py update` receive
  `.github/workflows/release-please.yml` cleanly (and drop release exclusions
  from their managed `.gitignore` block in `release` mode) without failing CI.
- Project-owned `release-please-config.json`, `.release-please-manifest.json`,
  and `CHANGELOG.md` remain tracked in git in both `normal` and `release` modes
  and are never overwritten by `bin/harness.py update`.
