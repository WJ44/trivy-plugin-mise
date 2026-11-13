# Harness Baseline Decision Records

Baseline tooling, workflow, and convention decisions inherited from the harness.
Project-specific decisions live in the parent [`decisions/`](../README.md)
directory with their own numbering.

| ID                                                                  | Title                                                                | Status | Finality    | Date       |
| ------------------------------------------------------------------- | -------------------------------------------------------------------- | ------ | ----------- | ---------- |
| [001](001-prettier-baseline-formatter.md)                           | Prettier as baseline formatter                                       | Active | Standard    | 2026-11-03 |
| [002](002-node-runtime.md)                                          | Node.js as JavaScript runtime for tooling                            | Active | Standard    | 2026-11-03 |
| [003](003-editorconfig-for-shared-settings.md)                      | Shared settings in .editorconfig                                     | Active | Standard    | 2026-11-03 |
| [004](004-shfmt-for-shell.md)                                       | shfmt for shell files                                                | Active | Standard    | 2026-11-03 |
| [005](005-lefthook-runner.md)                                       | lefthook as hook and task runner                                     | Active | Standard    | 2026-11-03 |
| [006](006-tool-installation.md)                                     | Tool installation with mise                                          | Active | Standard    | 2026-11-03 |
| [007](007-tombi-for-toml.md)                                        | tombi for TOML files                                                 | Active | Standard    | 2026-11-03 |
| [008](008-shellcheck-for-shell.md)                                  | ShellCheck for shell scripts                                         | Active | Standard    | 2026-11-08 |
| [009](009-ruff-for-python.md)                                       | Ruff for Python                                                      | Active | Standard    | 2026-11-08 |
| [010](010-markdownlint-evaluation.md)                               | Markdownlint evaluation                                              | Active | Provisional | 2026-11-08 |
| [011](011-conventional-commits-and-commitlint.md)                   | Conventional Commits with commitlint                                 | Active | Standard    | 2026-11-08 |
| [012](012-pull-request-template-and-convention.md)                  | Pull request templates for humans and agents                         | Active | Standard    | 2026-11-08 |
| [013](013-issue-templates-and-autonomous-proposals.md)              | Issue templates and autonomous agent proposals                       | Active | Standard    | 2026-11-08 |
| [014](014-git-branch-naming-convention.md)                          | Git branch naming convention                                         | Active | Standard    | 2026-11-08 |
| [015](015-ci-pipeline.md)                                           | GitHub Actions CI pipeline                                           | Active | Standard    | 2026-11-08 |
| [016](016-issue-label-taxonomy-and-sync.md)                         | Issue label taxonomy and synchronization                             | Active | Standard    | 2026-11-09 |
| [017](017-renovate-configuration.md)                                | Renovate for automated dependency updates                            | Active | Standard    | 2026-11-13 |
| [018](018-agent-lifecycle-hooks.md)                                 | Agent lifecycle hooks for post-edit formatting and linting           | Active | Standard    | 2026-11-09 |
| [019](019-agent-pr-review-process.md)                               | Agent PR review process                                              | Active | Standard    | 2026-11-09 |
| [020](020-subagent-delegation-for-bulk-reads.md)                    | Subagent delegation and model tier selection for bulk reads          | Active | Standard    | 2026-11-09 |
| [021](021-software-bill-of-materials-and-vulnerability-scanning.md) | Software Bill of Materials (SBOM) and dependency scanning with Trivy | Active | Standard    | 2026-11-09 |
| [022](022-multi-issue-orchestration.md)                             | Multi-issue and autonomous task queue orchestration                  | Active | Provisional | 2026-11-11 |
| [023](023-dev-toolchain-sbom-and-scanning.md)                       | Separate dev toolchain from shipped dependencies in SBOM and scan    | Active | Standard    | 2026-11-13 |
| [024](024-decision-records-format-and-workflow.md)                  | Decision records in decisions/ with finality and options considered  | Active | Standard    | 2026-11-13 |
| [025](025-unattended-agent-execution-in-sandbox.md)                 | Unattended agent execution inside the sandbox                        | Active | Locked      | 2026-11-13 |
| [026](026-agent-context-harness-namespace.md)                       | Separate harness and project docs in agent-context/                  | Active | Standard    | 2026-11-13 |
