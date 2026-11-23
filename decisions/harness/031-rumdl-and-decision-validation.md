# 031: rumdl Markdown linter and decision record validator

- **Status:** Active
- **Finality:** Standard
- **Scope:** `*.md`, `mise.toml`, `mise.lock`, `.rumdl.toml`, `lefthook.yml`,
  `bin/validate-decisions.py`, `bin/harness.py`, `.github/workflows/ci.yml`,
  `decisions/`
- **Date:** 2026-11-23

## Context

[Decision 010](010-markdownlint-evaluation.md) (`Provisional`) previously
declined `markdownlint-cli2` because its default rules conflicted with Prettier
and it lacked built-in relative file link checks. Relying on Prettier alone left
structural Markdown defects uncaught—including broken relative file links,
invalid heading anchor fragments, duplicate heading anchor collisions, and
missing code-fence language specifiers—while agents repeatedly wrote ad-hoc
scripts that tripped over examples inside code blocks or spans. Separately,
repository-specific Decision Record headers, required sections, and index tables
in `decisions/` and `decisions/harness/` lacked automated validation.

## Decision

Supersede [010](010-markdownlint-evaluation.md) by adopting **`rumdl`** for
general Markdown linting and **`bin/validate-decisions.py`** for Decision Record
structure and index validation:

1. **`rumdl` baseline (`aqua:rvben/rumdl`):** Pin `rumdl` in `mise.toml` /
   `mise.lock`, track `.rumdl.toml` as harness-owned in `bin/harness.py`, and
   wire `rumdl check --fix` (`lint`) and `rumdl check` (`lint-check`) into
   `lefthook.yml` for `*.md`.
2. **Minimal rules, flavors, and output formats:** Keep `rumdl` defaults
   (including relative links, cross-file fragments, frontmatter spacing, and
   code-fence languages) and default `standard` flavor (which `gfm` and `github`
   alias to, with no per-file flavor overrides needed in the baseline harness).
   Enable structural and safety opt-in rules (`MD070`, `MD080`, `MD083`,
   `MD087`, `MD091`, `MD095`), configure only minimal Prettier-compatibility
   adjustments (`MD013` code blocks; `MD076` loose-list compatibility is handled
   by `rumdl` defaults as of `v0.3.5`), and scope per-file ignores strictly to
   template and fragment files (`CLAUDE.md`, `*.template.md`, `*.project.md`, PR
   templates, `templates/decisions-README.md`). Keep `text` output locally and
   in agent hooks, and set `RUMDL_OUTPUT_FORMAT=github` in GitHub Actions CI for
   native PR annotations.
3. **Decision Record validator (`bin/validate-decisions.py`):** Keep Decision
   Record metadata in clickable Markdown bullet lists and validate H1 IDs,
   metadata bullets, required H2 sections, and `README.md` index table sync via
   a self-contained stdlib Python script (`SHARED_BIN_FILES` in
   `bin/harness.py`, scoped in `lefthook.yml` to `decisions/*.md` and
   `decisions/harness/*.md`).

## Options Considered

- **`rumdl` + focused `bin/validate-decisions.py` (chosen):** Provides fast Rust
  CommonMark-aware Markdown linting, cross-file link and GFM anchor validation,
  safe autofixes, and native GitHub CI annotations without Node/plugin bloat,
  while keeping repo-specific Decision Record metadata checks in a small
  zero-dependency Python script.
- **`markdownlint-cli2` (`npm:markdownlint-cli2`):** Rejected again because
  built-in fragment checks remain intra-file only and relative file path checks
  require third-party plugins.
- **`lychee` (`aqua:lycheeverse/lychee`):** Rejected because it checks links and
  anchors only and does not cover general Markdown structure or autofixes.
- **Custom Python script for all Markdown linting:** Rejected because
  re-implementing CommonMark AST parsing and GFM anchor slugification duplicates
  mature `rumdl` capabilities and misses structural rules.
- **YAML frontmatter for Decision Records + `rumdl` `MD072`:** Rejected because
  `MD072` enforces alphabetical key sorting globally (conflicting with issue
  templates and downstream frontmatter schemas), does not require frontmatter to
  exist, and prevents `rumdl` and GitHub from validating and rendering clickable
  `Superseded by` links in Markdown metadata.
- **Opinionated `rumdl` opt-in rules (`MD060`, `MD063`, `MD072`, `MD073`,
  `MD082`, `MD084`, `MD088`, `MD093`, `MD096`) and `editorconfig = true`:**
  Evaluated and left disabled because they overlap Prettier, conflict with
  intentional sentence-case headings, inline-code headings, em-dashes,
  frontmatter order, or ZWJ emojis (`MD084` strict mode), or cause config
  warnings against `.editorconfig`'s Markdown trailing-whitespace setting.

## Consequences

- Every `.md` edit is checked locally, via post-edit agent hooks, in pre-commit,
  and in CI for broken relative links, invalid heading anchors, duplicate
  anchors, missing code-fence languages, and structural issues.
- Every edit under `decisions/` or `decisions/harness/` immediately validates
  Decision Record schema compliance, `Superseded by` targets, and `README.md`
  index synchronization (skipping absent `decisions/harness/` in release-mode
  CI).
- Live external `http://` / `https://` URL reachability remains out of scope
  because offline/sandboxed environments block unapproved external hosts.
