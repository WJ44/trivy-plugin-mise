{{project:intro}}

## Subagent delegation

Keep the main context lean and reserve the primary model's token budget for
planning, reasoning, and editing:

- **Read directly** in the main session for targeted lookups (reading 1–3 known
  files or `rg`/`grep` search queries with small output) or files you are about
  to edit directly.
- **Delegate to a read-only subagent** (`research` via `invoke_subagent` in
  Antigravity; `Explore` via `Agent`/`Task` in Claude Code) for broad codebase
  exploration, reading more than 3–4 files, or scanning/summarizing large files
  or logs where raw contents are not needed in the main context. In Antigravity,
  `research` runs without network access, so tasks needing `gh` or network tools
  stay in the main session—or, for clean-context PR reviews, use
  `TypeName: "self"` per
  [Pull requests](agent-context/harness/pull-requests.md).
- **Select a smaller model tier:** When the main session runs on a large/capable
  model, explicitly set a smaller, faster model for read-only exploration
  subagents (`Model: "flash"` by default or `Model: "flash_lite"` for trivial
  high-volume extraction in Antigravity; `model: "haiku"` in Claude Code)
  instead of inheriting the main session's model.
- **Return synthesized findings:** Instruct the subagent to return a concise
  summary with relevant file paths and line numbers rather than dumping raw file
  contents into the main conversation.

{{project:sections}}

## Reference

Read these when the topic comes up; they are not loaded automatically.

- [Commits and branches](agent-context/harness/commits.md) — git branch naming,
  conventional commit conventions, and commit message requirements. Read before
  creating branches or git commits.
- [Agent tool configuration](agent-context/harness/agent-configuration.md) —
  where agent and harness tool config belongs and which format to pick. Read
  before adding or moving any agent or harness configuration file or setting.
- [Sandbox](agent-context/harness/sandbox.md) — prefer Context7 MCP
  (`resolve-library-id` then `query-docs`) for version-specific external
  library/tool docs before falling back to general web search, use internet
  access as normal, and ask for host approval when blocked by the allowlist.
  Read when looking up external docs or when a download or request is blocked.
- [Formatting and linting](agent-context/harness/formatting-and-linting.md) —
  requirements every formatter or linter must meet. Read before adding or
  changing either.
- [Testing](agent-context/harness/testing.md) — when to add or update tests,
  where harness script tests live (`tests/`), how to run them (`mise run test`),
  and how downstream `init`/`update`/`release` handles `tests/`. Read before
  modifying harness scripts (`bin/`, `hooks/`, `host/`, `setup-user.sh`) or
  adding/running tests.
- [Pull requests](agent-context/harness/pull-requests.md) — pull request
  template structure, branch conventions, reviewer hand-off requirements, and
  agent PR review workflow. Read before opening, updating, or reviewing a pull
  request.
- [Issues](agent-context/harness/issues.md) — issue templates, labels,
  autonomous agent proposals, and human approval workflow. Read before opening
  or working on an issue.
- [Issue refinement](agent-context/harness/issue-refinement.md) — interactive
  human-in-the-loop workflow for refining issues and proposals. Read only when
  asked to refine an issue.
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
- [Releasing](agent-context/harness/releasing.md) — Semantic Versioning (pre-1.0
  vs. `1.0.0+`), `release-please` Release PR review workflow,
  `RELEASE_PLEASE_TOKEN`, and customizing release workflows. Read before
  authoring releasable/breaking changes or reviewing a Release PR.
{{project:references}}

### Keeping this file small

`AGENTS.md` is loaded into context in full on every session, so it holds only
what is always relevant. Agent operational rules and workflows go in
`agent-context/` (project-specific directly under `agent-context/`, baseline
harness docs under `agent-context/harness/`) linked from the list above with a
one-line description of when to read them. Human-focused usage and conceptual
documentation lives in `docs/` (project-specific, indexed in
[`README.md`](README.md)) and `docs/harness/` (baseline harness docs such as
[`harness-cli.md`](docs/harness/harness-cli.md), linked from `README.md` in
normal mode and kept on disk locally in release mode); agents only need to read
those files when learning how a tool or feature is used.

`AGENTS.md` and `README.md` are rendered from `AGENTS.template.md` /
`README.template.md` and `agent-context/*.project.md` via
`bin/harness.py render` (optional check: `bin/harness.py render --check`); edit
the template or `*.project.md` sources and re-render rather than editing the
generated files directly.
