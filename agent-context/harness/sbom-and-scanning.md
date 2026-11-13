# Software Bill of Materials (SBOM) and Dependency Scanning

This repository uses [Trivy](https://github.com/aquasecurity/trivy) to generate
a standard [CycloneDX](https://cyclonedx.org/) Software Bill of Materials (SBOM)
and scan dependencies for known vulnerabilities (CVEs) and license compliance.

## Tooling and Configuration

Trivy is pinned in `mise.toml` and locked in `mise.lock`. Configuration and
execution are coordinated through:

- `bin/mise-sbom`: Prototype CLI helper script coordinating CycloneDX SBOM
  generation and multi-tier dependency scanning prior to extraction into
  [`WJ44/trivy-plugin-mise`](https://github.com/WJ44/trivy-plugin-mise).
- `trivy.yaml`: Defines repository defaults for filesystem scanning
  (`trivy fs`), enabling both vulnerability (`vuln`) and license (`license`)
  scanners, exiting non-zero (`exit-code: 1`) on unaccepted findings, and
  pointing to `.trivyignore.yaml`.
- `.trivyignore.yaml`: The explicit risk-acceptance ledger for vulnerabilities
  and licenses.

## Shipped Dependencies (`components[]`) vs. Toolchain (`formulation[]`)

In accordance with supply chain industry standards (CISA SBOM Minimum Elements,
EU Cyber Resilience Act) and CycloneDX 1.7:

- **Shipped Dependencies (`components[]`):** Product SBOMs list only software
  dependencies that ship with or link into the distributable application
  (discovered from `package.json`, `go.mod`, `Cargo.lock`, `uv.lock`, etc.).
  Repositories with no distributed runtime dependencies leave `components[]`
  empty.
- **Build & Dev Toolchain (`formulation[]`):** Build and development tools
  managed by `mise` (`mise.toml`, `mise.lock`, and transitive npm locks under
  `.mise/locks/`) are injected into the CycloneDX **`formulation[]`** section
  (Manufacturing BOM / formula components). This ensures:
  1. Downstream products do not falsely claim to ship build-time linters or
     formatters (`shellcheck`, `prettier`, `commitlint`, `lefthook`, `ruff`).
  2. Standalone CLI binaries with copyleft licenses (such as GPL-3.0
     `shellcheck`) do not trigger false-positive license violations in CI as if
     copyleft code were bundled into the shipped product.
  3. Every locked tool records full platform checksums, download URLs,
     provenance attestations (`github-attestations`, `cosign`), and transitive
     ecosystem package dependencies.

## Local Usage

### Generate CycloneDX SBOM

```bash
mise run sbom
```

The output file `sbom.cdx.json` is ignored by `.gitignore` and must not be
committed to version control.

### Run Vulnerability and License Checks

```bash
mise run scan
```

`bin/mise-sbom scan` performs a two-tier scan:

1. **Repository root:** Scans repository dependencies for both vulnerabilities
   and licenses (`--scanners vuln,license`) per `trivy.yaml`.
2. **Toolchain lockfiles:** Materializes `.mise/locks/` (linking
   `aube-lock.yaml` as `pnpm-lock.yaml`) and scans tool dependencies for
   vulnerabilities (`--scanners vuln`), respecting `.trivyignore.yaml`.

A non-zero exit code indicates unaccepted vulnerabilities or license violations.

## Risk Acceptance Ledger (`.trivyignore.yaml`)

Every security or license exception must be explicitly recorded and justified in
`.trivyignore.yaml`. Silent suppression is disallowed.

### Ledger Structure

```yaml
vulnerabilities:
  - id: CVE-2026-1234
    statement: "Detailed explanation of why this risk is accepted or mitigated."
    expired_at: 2026-12-31 # Optional expiration date (YYYY-MM-DD)
    paths: # Optional file path restrictions
      - "package.json"

licenses:
  - id: MPL-2.0
    statement:
      "Permissible weak copyleft license reviewed and approved for dynamic
      linking."
```

### Required Fields for Exceptions

1. **`id`**: The CVE identifier (e.g. `CVE-2026-1234`) or SPDX license
   identifier (e.g. `MPL-2.0`).
2. **`statement`**: A concrete rationale explaining why the risk is accepted,
   mitigated by compensating controls, or tracked for upstream resolution.
3. **`expired_at`** _(recommended)_: An ISO 8601 date (`YYYY-MM-DD`) after which
   the exception expires, forcing re-evaluation.
4. **`paths`** _(optional)_: Scopes the exception to specific manifest or lock
   files.

## CI Automation

Scanning is automated in `.github/workflows/ci.yml`:

- **Triggers**: Runs on pushes to `main`, pull requests, and a daily scheduled
  cron trigger (`0 6 * * *`) so newly disclosed CVEs are detected even when no
  code changes occur.
- **Artifacts**: Generates `sbom.cdx.json` and uploads it as a workflow artifact
  via `actions/upload-artifact` (pinned by commit SHA digest).
- **Quality gate**: Scans the filesystem using repository configuration; any
  unaccepted vulnerability or license failure blocks the pipeline.
