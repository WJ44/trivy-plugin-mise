# 013: Issue templates and autonomous agent proposals

- **Status:** Active
- **Finality:** Standard
- **Scope:** `.github/ISSUE_TEMPLATE/`, `agent-context/harness/issues.md`
- **Date:** 2026-11-08

## Context

Issues serve as the contract between humans and autonomous agents. Human-created
issues are typically picked up by agents, so bounded scope and concrete
task-specific acceptance criteria are essential. Agents also file issues when
discovering out-of-scope problems mid-task, which require self-contained context
and a human approval gate before implementation. Additionally, templates must
work in both the GitHub web UI and `gh issue create --template` (which supports
Markdown `.md` templates, not YAML issue forms).

## Decision

Use Markdown issue templates in `.github/ISSUE_TEMPLATE/` for bug reports,
feature/task requests, design decisions, and autonomous agent proposals, and
gate autonomous proposals behind an explicit human approval step (`approved`
label plus `- [x] Approved to proceed` checkbox).

## Options Considered

- **Markdown (`.md`) templates in `.github/ISSUE_TEMPLATE/` (chosen):** Work
  identically in the GitHub web UI and via `gh issue create --template`.
- **GitHub YAML Issue Forms (`.yml`):** Rejected because
  `gh issue create --template` does not support YAML issue forms for CLI/agent
  creation.

## Consequences

Templates work uniformly across the web UI and `gh` CLI. Acceptance criteria
stay focused on task-specific verification, bounded scope sections prevent agent
drift, and autonomous agent proposals wait for explicit human approval.
