# 011: Conventional Commits with commitlint

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.commitlintrc.json`, `lefthook.yml` (`commit-msg`), `mise.toml`,
  `agent-context/harness/commits.md`
- **Date:** 2026-11-08

## Context

Without a uniform commit message format, automated changelogs, semantic
versioning, and git history browsing become inconsistent across human and agent
contributions.

## Decision

Adopt the [Conventional Commits](https://www.conventionalcommits.org/)
specification, enforced via `@commitlint/cli` on `commit-msg`, and require
breaking changes to include both the `!` header flag and a `BREAKING CHANGE:`
footer.

## Options Considered

- **Conventional Commits enforced via `@commitlint/cli` (chosen):** Widely
  adopted standard understood by agents, humans, and release/dependency tools
  (such as Renovate), with immediate local feedback on invalid messages.
- **Documentation-only commit guidelines (no hook):** Rejected because agents
  and humans can drift from formatting rules without an automated check.

## Consequences

All commits use standard prefixes (`feat`, `fix`, `chore`, `docs`, `refactor`,
`style`, `test`, `ci`, `perf`, `build`, `revert`), and non-conforming commit
messages are rejected at commit time.
