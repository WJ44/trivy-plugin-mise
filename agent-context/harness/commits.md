# Commits and branches

This document outlines git conventions for branch naming and commit messages.

## Branch naming

Branches use a prefix matching the [allowed commit types](#allowed-types) below:

```
<type>/<name>
```

### Branch naming rules

- **Format**: Lowercase alphanumeric characters and hyphens only (`[a-z0-9-]`).
  Do not use underscores, uppercase letters, or special characters.
- **Issue references**: When addressing a specific issue, include the issue
  number in the name (e.g. `feat/8-branch-naming` or `fix/42-token-leak`).
- **Descriptive slugs**: Keep the branch slug concise (2–4 words) and
  descriptive of the goal or deliverable.

## Commits

Every commit message in this repository must follow the
[Conventional Commits](https://www.conventionalcommits.org/) specification.
Commit messages are validated at commit time via a git `commit-msg` hook using
`commitlint`.

### Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Header rules

- **type** must be lowercase and one of the allowed types.
- **description** (subject) must be lowercase, must not be empty, and must not
  end with a period (`.`).
- Header line length must not exceed 100 characters.

### Allowed types

| Type       | Description                                                  |
| ---------- | ------------------------------------------------------------ |
| `feat`     | A new capability or feature                                  |
| `fix`      | A bug fix or correction                                      |
| `chore`    | Tooling changes, dependency pinning, or maintenance          |
| `docs`     | Documentation changes only                                   |
| `refactor` | Code refactoring without changing observable behavior        |
| `style`    | Formatting or whitespace changes (no functional code change) |
| `test`     | Adding or fixing test suites                                 |
| `ci`       | CI configuration or automation scripts                       |
| `perf`     | Performance improvements                                     |
| `build`    | Build system changes                                         |
| `revert`   | Reverting a previous commit                                  |

### Scopes (optional)

An optional noun phrase wrapped in parentheses clarifying the affected area,
e.g. `feat(setup): ...` or `fix(token-server): ...`.

### Body (optional)

Separated from the header by a blank line. Explains _why_ the change was made
and context that isn't obvious from the diff alone.

### Footers (optional)

Separated from the body by a blank line. Used for:

- Issue references: `Refs #1`, `Closes #2`, `Fixes #3`.
- Breaking change description: `BREAKING CHANGE: <explanation>`.

### Breaking changes

Always do both:

1. **Header flag**: Append `!` before the `:` (e.g. `feat!: ...` or
   `feat(scope)!: ...`) so breaking changes are immediately visible in compact
   views like `git log --oneline` and PR titles.
2. **Footer explanation**: Add a `BREAKING CHANGE: <explanation>` footer
   explaining what broke and how to migrate, providing the context changelog
   generators and consumers need.
