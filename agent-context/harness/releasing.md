# Releasing

Projects initialized from `harness` inherit automated SemVer releases via
[`release-please`](https://github.com/googleapis/release-please)
(`.github/workflows/release-please.yml`, `release-please-config.json`,
`.release-please-manifest.json`, and `CHANGELOG.md`). All four files stay
tracked in git in both `normal` and `release` modes; the workflow is
harness-owned while the configuration, manifest, and changelog are owned by the
project.

## Semantic Versioning & pre-1.0 bump rules

Projects start from `{ ".": "0.0.0" }` in `.release-please-manifest.json` with
`bump-minor-pre-major: true` and `bump-patch-for-minor-pre-major: true` in
`release-please-config.json`:

| Change                | Prefix / marker             | Pre-1.0 (`0.x.y`) bump | Post-1.0 (`>= 1.0.0`) bump |
| --------------------- | --------------------------- | ---------------------- | -------------------------- |
| Bug fix / improvement | `fix:` (`perf:`, `revert:`) | Patch (`0.x.y+1`)      | Patch (`X.Y.Z+1`)          |
| New feature           | `feat:`                     | Patch (`0.x.y+1`)      | Minor (`X.Y+1.0`)          |
| Breaking change       | `!` + `BREAKING CHANGE:`    | Minor (`0.x+1.0`)      | Major (`X+1.0.0`)          |

Routine maintenance and internal commits (`chore:`, `docs:`, `ci:`, `refactor:`,
`style:`, `test:`, `build:`) accumulate without opening a Release PR unless they
carry breaking markers or an explicit `Release-As: X.Y.Z` commit footer.

- **Promoting to `1.0.0`:** When the project is ready for its first stable major
  release, trigger it with a `Release-As: 1.0.0` commit footer (or let the
  maintainer approve the promotion commit). After `1.0.0` is tagged, standard
  SemVer bump rules apply automatically.

## Authoring PRs and commits for `release-please`

- **Align the PR title with the bump:** Squash-merged PRs use the PR title as
  the commit header on `main`. Always set the PR title to the intended
  Conventional Commit type (including `!` when breaking).
- **Write actionable `BREAKING CHANGE:` footers:** Include both `!` in the
  header and a `BREAKING CHANGE: <migration steps>` footer (wrapped at 100
  characters per [Commits and branches](commits.md)). `release-please` copies
  these notes into `CHANGELOG.md`.

## `RELEASE_PLEASE_TOKEN` & workflow guard

`.github/workflows/release-please.yml` authenticates with
`secrets.RELEASE_PLEASE_TOKEN` (a repository-scoped fine-grained Personal Access
Token with `Contents: Read and write`, `Pull requests: Read and write`,
`Issues: Read and write`, and `Metadata: Read-only` so Release PRs trigger
`on: pull_request` CI and created tags/releases can trigger downstream
workflows).

Before invoking `googleapis/release-please-action`, the workflow runs a guard
step checking that `release-please-config.json`,
`.release-please-manifest.json`, and `secrets.RELEASE_PLEASE_TOKEN` are present.
If any is missing, it emits a `::notice::` and skips `release-please` cleanly
without failing CI.

## Reviewing and merging `release-please` Release PRs

When releasable commits merge to `main`, `release-please` opens or updates a
Release PR (`chore(main): release X.Y.Z`) updating `CHANGELOG.md`,
`.release-please-manifest.json`, and any configured `extra-files`.

- **Do not auto-merge Release PRs:** Cutting a release is a human decision; stop
  and wait for human approval unless explicitly asked to prepare or merge a
  Release PR.
- **When reviewing or preparing a Release PR:**
  1. Confirm the computed SemVer bump matches the changes merged since the last
     `vX.Y.Z` tag.
  2. Verify `CHANGELOG.md`, `.release-please-manifest.json`, and any configured
     `extra-files` match the target version.
  3. Review `CHANGELOG.md` entries—especially `### ⚠ BREAKING CHANGES`—and
     refine migration steps on the Release PR branch if needed.
  4. Verify CI passes before merging.
  5. After a human merges the Release PR, `release-please` creates the `vX.Y.Z`
     tag and GitHub Release.

## Customizing and extending releases

- **Language `release-type` and `extra-files`:** Starter templates use
  `"release-type": "simple"`. Customize `release-please-config.json` to switch
  to a language-specific `release-type` (such as `python`, `node`, `go`, or
  `rust`) or add `extra-files` entries so `release-please` updates project
  version files automatically.
- **Artifact build & publish workflows:** Keep ecosystem-specific build,
  packaging, checksum, provenance, or registry publishing workflows in separate
  project workflow files triggered by `on: push: tags: ["v*"]` or
  `on: release: types: [published]`. See
  [Releasing](../../docs/harness/releasing.md) for human setup instructions.
