# Pull requests

Pull requests in this repository are typically created by autonomous agents and
reviewed by humans who may have had zero prior involvement in the task.
Reviewers need to be able to evaluate the PR and make a confident decision in 3
to 5 minutes (e.g. during a coffee break) without digging through raw logs or
session transcripts.

Occasional PRs are also opened by humans; the same branch and title conventions
apply, with a lighter-weight template.

## Branch naming

Pull requests must be opened from branches following the conventions documented
in [Commits and branches](commits.md#branch-naming) (`<type>/<name>`).

## Title format

PR titles must follow the [Conventional Commits](commits.md) format:

```
<type>[optional scope]: <description>
```

Example: `feat(setup): split setup script into system and user scripts (#11)`

## Templates

Templates live in `.github/PULL_REQUEST_TEMPLATE/`:

- [`pull_request_template.md`](../../.github/PULL_REQUEST_TEMPLATE/pull_request_template.md)
  (Default): Automatically loaded by the GitHub web UI for human-authored PRs.
  Kept minimal (`Context & Changes`, `Notes / Verification`).
- [`agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md): Used by agents
  creating PRs via `gh pr create`. Structured as a self-contained review packet
  for an uninvolved human reviewer.

## Agent PR requirements

When creating a pull request via `gh pr create`, read and follow
[`.github/PULL_REQUEST_TEMPLATE/agent.md`](../../.github/PULL_REQUEST_TEMPLATE/agent.md):

- **Context & Goal:** 1–2 sentences summarizing the problem and goal, linking
  relevant issues (`Closes #<id>`, `Fixes #<id>`, or `Refs #<id>`).
- **Summary of Changes:** Concise bullets grouped by behavior or concept rather
  than repeating the file-by-file diff.
- **Reviewer Hand-off:** Context transfer for a reviewer who was not in the
  session — non-obvious decisions/trade-offs, files needing close scrutiny, and
  any caveats, follow-up `Agent Proposal` issues, or items untestable in the
  sandbox (guard against the "looks done" trap; state "None" if none).
- **Acceptance Criteria & Verification:** Mirror the issue's acceptance criteria
  checklist (`- [x] ...`) and briefly note how behavior was verified in practice
  (do not pad with boilerplate `format-check`/`lint-check` checkboxes or raw
  terminal logs).

## Agent PR review workflow

Pull requests created by an autonomous agent must be reviewed by a second agent
running with a clean context before a human makes the final merge or revision
decision. Reviewing in the authoring agent's existing session risks confirmation
bias from accumulated reasoning, while waiting for a human to manually kick off
a review adds friction to the coffee-break review workflow.

### 1. Post-PR review hand-off

Immediately after opening a pull request (`gh pr create`), the authoring agent
must spawn a subagent with a clean context (e.g. `invoke_subagent` in
Antigravity or the `Agent` tool in Claude Code) to review the PR:

- Pass **only** the PR number and instructions to read and follow the review
  guide in `agent-context/harness/pull-requests.md`.
- Do **not** pass the authoring agent's implementation reasoning, session
  transcript, or justifications, so the reviewing subagent evaluates the PR on
  its own merits.

### 2. Review rubric (points of attention)

The reviewing subagent inspects the PR description (`gh pr view <pr>`), the diff
(`gh pr diff <pr>`), status checks (`gh pr checks <pr>`), and the linked issue
(`gh issue view <issue>`) against four points of attention:

1. **Acceptance criteria & scope boundaries:** Verify that every acceptance
   criterion on the linked issue is genuinely satisfied, and that the diff stays
   strictly within the declared scope and non-goals without unrelated
   refactoring.
2. **Conventions & Decision Records:** Verify that changes comply with
   [`agent-context/`](..) guidelines and active [`decisions/`](../../decisions/)
   records (`Standard` and `Locked` finality levels; see
   [Decisions](decisions.md)). Confirm that a new or updated Decision Record is
   included (and indexed in [`decisions/README.md`](../../decisions/README.md))
   if the PR introduces or changes a convention, tool, or workflow.
3. **Correctness, edge cases & security:** Check logic, error handling, test
   coverage, edge cases, and security/credential boundaries.
4. **PR hand-off accuracy:** Verify that the `Reviewer Hand-off` section is
   accurate and guards against the "looks done" trap (any stubbed, deferred, or
   sandbox-untestable items are explicitly called out, and required decision
   records or follow-up proposals are present).

### 3. Review output format & delivery

- **Read-only boundary:** The reviewing subagent only reports findings—it must
  **not** modify files, create commits, push changes, or merge the PR.
- **Delivery mechanism:** The reviewing subagent posts a concise, structured
  review comment on the PR via `gh pr review <pr> --comment` (or
  `gh pr comment <pr>`). Do **not** use `gh pr review --approve` or
  `--request-changes`, as all agents share the same GitHub App bot identity and
  GitHub disallows approving or requesting changes on one's own PR.
- **Severity grouping:** Group findings by severity so a human can scan the
  review quickly:
  - **`Blocking`** — Unmet acceptance criteria, defects, security/boundary
    violations, or broken conventions that must be resolved before merge.
  - **`Needs Human Decision`** — Ambiguities, design trade-offs, or scope
    questions requiring human judgment.
  - **`Suggestion / Nit`** — Non-blocking improvements or minor polish.
  - **Explicit pass:** If no issues are found, state an explicit pass confirming
    the rubric checks were verified.

### 4. Human intervention gate

- Once the review comment is posted on the PR, both the reviewing subagent and
  the authoring agent **must stop and wait for human intervention**.
- Neither the authoring agent nor a new agent may act on the review findings
  (such as pushing fix-up commits) until a human has reviewed the PR and
  findings and approved or provided direction.

## Handling Renovate dependency PRs

Automated dependency updates are managed by Renovate.

Intervene on a Renovate PR only when CI fails or a lockfile is out of sync (for
example, a formatter/linter bump requires reformatting existing files, a new CVE
fails `mise run scan`, or another ecosystem lacks lockfile support):

1. **Check out the PR branch**: `gh pr checkout <pr-number>`.
2. **Resolve the failure**: Refresh lockfiles (`mise lock`) or formatting
   (`mise x -- lefthook run format`) as needed, or investigate scan/lint errors.
3. **Verify locally**: Run
   `mise x -- lefthook run format-check && mise x -- lefthook run lint-check`
   (and `mise run scan` if security checks failed).
4. **Commit and verify CI**: Commit any fixes using Conventional Commits with
   type `chore` (e.g. `chore(deps): format for prettier update`), push, and
   confirm CI passes on the PR branch.
