# 028: yq validation for .trivyignore.yaml rules

- **Status:** Active
- **Finality:** Standard
- **Scope:** `mise.toml`, `mise.lock`, `lefthook.yml`,
  `bin/validate-trivyignore.yq`, `.trivyignore.yaml`, `agent-context/harness/`
- **Date:** 2026-11-16

## Context

Repository policy
([021](021-software-bill-of-materials-and-vulnerability-scanning.md)) requires
every accepted finding in `.trivyignore.yaml` to carry an `id` and a non-empty
`statement`. However, Trivy treats `statement` (and missing `id`) permissively
when loading `.trivyignore.yaml`, while already validating YAML syntax,
duplicate keys, structural types, and expiration timestamps at scan time. Rather
than re-implementing Trivy's own parser, linting only needs to enforce the
repository-specific required fields added on top of Trivy.

## Decision

Pin `yq` in `mise.toml` and `mise.lock`, store the validation expression in
`bin/validate-trivyignore.yq`, and invoke it via `lefthook.yml` so every ignore
entry must be a mapping with a non-empty, non-whitespace `id` and `statement`.

## Options Considered

- **Pure `yq` check (`bin/validate-trivyignore.yq`, chosen):** Single static Go
  binary pinned with multi-platform checksums and provenance in `mise.lock` (no
  Python or npm transitive dependencies). Handles empty/comment-only files and
  YAML aliases/merge keys cleanly, keeps `lefthook.yml` concise without a custom
  wrapper script or external schema validator, and provides a general-purpose
  YAML tool for the repository.
- **`yq` + custom Python validation script (earlier draft, rejected):**
  Initially paired `yq` with a full stdlib Python schema validator covering
  category/key allowlists, duplicate keys, and expiration formats. Replaced
  after review because Trivy already validates those rules at scan time.
- **Custom JSON/YAML Schema validator (`jsonschema`, `check-jsonschema`,
  `ajv-cli`):** Rejected because maintaining a separate schema file and a
  single-purpose schema CLI adds overhead, and evaluated schema validators
  either pulled extra ecosystem dependencies or failed on empty files and YAML
  merge keys.
- **`cue`:** Rejected because enforcing a single required-field rule does not
  justify maintaining a separate schema file and dedicated `cue` binary rather
  than standard YAML/JSON tooling.

## Consequences

Every edit to `.trivyignore.yaml` is checked for non-empty `id` and `statement`
fields via the standard `lefthook` lint hooks and CI without maintaining custom
Python validation code, while Trivy remains responsible for validating YAML
types, duplicate keys, and expiration values during security scans.
