# trivy-plugin-mise

Project description.

## Subagent delegation

Keep the main context lean and reserve the primary model's token budget for
planning, reasoning, and editing:

- **Read directly** in the main session for targeted lookups (1–3 known files)
  or files you are about to edit directly.
- **Delegate to a read-only subagent** (`research` via `invoke_subagent` in
  Antigravity; `Explore` via `Agent`/`Task` in Claude Code) for broad codebase
  exploration, reading more than 3–4 files, or scanning/summarizing large files
  or logs where raw contents are not needed in the main context.
- **Select a smaller model tier:** When the main session runs on a large/capable
  model, explicitly set a smaller, faster model for read-only exploration
  subagents (`Model: "flash"` by default or `Model: "flash_lite"` for trivial
  high-volume extraction in Antigravity; `model: "haiku"` in Claude Code)
  instead of inheriting the main session's model.
- **Return synthesized findings:** Instruct the subagent to return a concise
  summary with relevant file paths and line numbers rather than dumping raw file
  contents into the main conversation.

## Reference

Read these when the topic comes up; they are not loaded automatically.

- [Commits and branches](agent-context/harness/commits.md) — git branch naming,
  conventional commit conventions, and commit message requirements. Read before
  creating branches or git commits.
- [Agent tool configuration](agent-context/harness/agent-configuration.md) —
  where agent and harness tool config belongs and which format to pick. Read
  before adding or moving any agent or harness configuration file or setting.
- [Sandbox](agent-context/harness/sandbox.md) — use internet access and web
  search as normal (just as outside the sandbox) and ask for host approval when
  blocked by the allowlist. Read when a download or request is blocked or host
  approval is needed.
- [Formatting and linting](agent-context/harness/formatting-and-linting.md) —
  requirements every formatter or linter must meet. Read before adding or
  changing either.
- [Pull requests](agent-context/harness/pull-requests.md) — pull request
  template structure, branch conventions, reviewer hand-off requirements, and
  agent PR review workflow. Read before opening, updating, or reviewing a pull
  request.
- [Issues](agent-context/harness/issues.md) — issue templates, labels,
  autonomous agent proposals, and human approval workflow. Read before opening
  or working on an issue.
- [Task queue orchestration](agent-context/harness/task-queue.md) — dispatcher
  and fresh-context subagent workflow for working through multiple issues/PRs or
  an open-ended backlog. Read only when asked to process multiple tasks or an
  open-ended queue.
- [Decisions](agent-context/harness/decisions.md) — decision records in
  `decisions/` and `decisions/harness/`, finality levels (`Provisional`,
  `Standard`, `Locked`), and how to record or revisit design decisions. Read
  before recording a decision or questioning an existing convention.
- [Software bill of materials and scanning](agent-context/harness/sbom-and-scanning.md)
  — generating CycloneDX SBOMs, scanning dependencies for CVEs and licenses with
  Trivy, and recording accepted risks in `.trivyignore.yaml`. Read before
  inspecting dependencies, checking CVEs/licenses, or updating ignore rules.

### Keeping this file small

`AGENTS.md` is loaded into context in full on every session, so it holds only
what is always relevant. Everything else goes in an `agent-context/` file
(project-specific directly under `agent-context/`, baseline harness docs under
`agent-context/harness/`) linked from the list above, with a one-line
description of when to read it.
