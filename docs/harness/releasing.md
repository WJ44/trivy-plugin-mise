# Releasing with `release-please`

Projects initialized from `harness` include automated
[Semantic Versioning 2.0.0](https://semver.org/) releases powered by
[`release-please`](https://github.com/googleapis/release-please):

- **Harness-owned workflow:**
  [`.github/workflows/release-please.yml`](../../.github/workflows/release-please.yml)
  is synced by `bin/harness.py init` and `update`.
- **Project-owned configuration & state:** `release-please-config.json` and
  `.release-please-manifest.json` are seeded from starter templates on
  `bin/harness.py init` (or `init --in-place`), and `CHANGELOG.md` is created by
  `release-please` on your first release. All four files stay tracked in git in
  both `normal` and `release` modes and are never overwritten by
  `bin/harness.py update`.

## Setup: `RELEASE_PLEASE_TOKEN`

[`.github/workflows/release-please.yml`](../../.github/workflows/release-please.yml)
authenticates with `secrets.RELEASE_PLEASE_TOKEN` rather than the built-in
`GITHUB_TOKEN`. Using a Personal Access Token allows `release-please` to open
Release PRs regardless of repository-level Actions PR-creation restrictions,
ensures Release PRs trigger `on: pull_request` checks in
[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml), and ensures
created tags and releases can trigger downstream publish workflows.

1. Create a **repository-scoped fine-grained Personal Access Token** for the
   target repository with permissions:
   - **Contents:** Read and write
   - **Pull requests:** Read and write
   - **Issues:** Read and write
   - **Metadata:** Read-only (granted automatically)
2. Add it under **Settings -> Secrets and variables -> Actions** as
   `RELEASE_PLEASE_TOKEN`.

### Safe Skip Guard

Before running `googleapis/release-please-action`, the workflow verifies that
`release-please-config.json`, `.release-please-manifest.json`, and
`secrets.RELEASE_PLEASE_TOKEN` are present. If any prerequisite is missing—for
example on a freshly initialized repo before `RELEASE_PLEASE_TOKEN` is added, or
on an existing downstream repo upgraded via `bin/harness.py update`—the workflow
emits a `::notice::` explaining what is missing and skips `release-please`
without failing `main` CI.

## Pre-1.0 vs. `1.0.0` Promotion

Starter templates seed `.release-please-manifest.json` at `0.0.0` with
`"bump-minor-pre-major": true` and `"bump-patch-for-minor-pre-major": true` in
`release-please-config.json`:

- **Pre-1.0 (`0.x.y`):**
  - `fix:` (`perf:`, `revert:`) and `feat:` commits bump the **patch** version
    (`0.x.y+1`).
  - Breaking changes (`!` in header + `BREAKING CHANGE:` footer) bump the
    **minor** version (`0.x+1.0`).
- **Promoting to `1.0.0`:** When the project is ready for a stable `1.0.0`
  release, add a `Release-As: 1.0.0` footer to a commit merged to `main` (for
  example via an empty commit or release-prep PR).
- **Post-1.0 (`>= 1.0.0`):** Standard SemVer bumps apply (`fix:` -> patch,
  `feat:` -> minor, `!` / `BREAKING CHANGE:` -> major).

Non-releasing commit types (`chore:`, `docs:`, `ci:`, `refactor:`, `style:`,
`test:`, `build:`) accumulate without opening a Release PR unless they carry a
breaking marker or `Release-As: X.Y.Z`.

## How `release-please` Runs & Manual Dispatch

On every push to `main`—and manually via **Actions -> Release Please -> Run
workflow** or `gh workflow run release-please.yml` (`workflow_dispatch`)—the
workflow inspects Conventional Commits since the last release tag:

1. When releasable commits land on `main`, `release-please` opens or updates a
   Release PR (`chore(main): release X.Y.Z`) updating `CHANGELOG.md`,
   `.release-please-manifest.json`, and any configured `extra-files`.
2. Review the proposed version bump and `CHANGELOG.md` entries (especially
   `### ⚠ BREAKING CHANGES` migration notes), verify CI passes, and merge when
   ready to cut the release.
3. Merging the Release PR causes `release-please` to create the `vX.Y.Z` git tag
   and GitHub Release.

## Customizing `release-please-config.json`

The starter config uses `"release-type": "simple"` targeting `CHANGELOG.md`.
Because `release-please-config.json` is project-owned, you can customize it
freely:

- **Ecosystem `release-type`:** Change `"release-type"` to match your stack (for
  example `python`, `node`, `go`, `rust`, or `simple`) so `release-please`
  updates package manifests (`pyproject.toml`, `package.json`, etc.).
- **Extra version files (`extra-files`):** Add `"extra-files"` entries (for
  example generic `# x-release-please-version` markers or JSON/YAML paths) to
  keep additional version constants synchronized.

## Triggering Project-Specific Build & Publish Workflows

Language- and ecosystem-specific artifact builds, checksums, provenance
attestations, and registry publishing belong in separate project workflow files
that hook onto the tags or GitHub Releases created by `release-please`:

- **Tag-triggered (`v*`):** Use `on: push: tags: ["v*"]` for build/publish jobs
  triggered when `release-please` pushes the release tag.
- **Release-triggered:** Use `on: release: types: [published]` to attach build
  artifacts or publish packages when the GitHub Release is published.

Both triggers require `RELEASE_PLEASE_TOKEN` (rather than `GITHUB_TOKEN`) so
tags and releases created by `release-please` trigger downstream GitHub Actions
workflows.
