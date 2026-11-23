# `trivy-plugin-mise` Usage Guide

`trivy-plugin-mise` bridges [`jdx/mise`](https://github.com/jdx/mise)
(`mise.toml`, `mise.lock`, and `.mise/locks/`) with
[`aquasecurity/trivy`](https://github.com/aquasecurity/trivy) to generate
[CycloneDX 1.7](https://cyclonedx.org/) Software Bills of Materials (SBOMs) and
run multi-tier vulnerability and license scans across both repository
dependencies and `mise`-managed developer toolchains.

---

## Installation & Invocation

`trivy-plugin-mise` is distributed as a single static binary with a root
`plugin.yaml` manifest, supporting two interchangeable installation and
invocation methods.

### Runtime Prerequisites

- **`trivy` on `PATH` _(required)_:** `trivy-plugin-mise` shells out to `trivy`
  (`trivy fs` and `trivy rootfs`) for filesystem extraction, vulnerability
  scanning, and license scanning.
- **`mise` on `PATH` _(required only for `scan --include-gobinary`)_:** When
  `--include-gobinary` is passed to `scan`, `trivy-plugin-mise` invokes
  `mise ls --json` in the target directory to discover active tool installation
  paths.

### Method 1: As a Trivy Plugin (`trivy mise ...`)

Install the plugin from GitHub using Trivy's plugin manager:

```bash
trivy plugin install github.com/WJ44/trivy-plugin-mise
```

Invoke subcommands through `trivy mise`:

```bash
trivy mise --help
trivy mise sbom --output sbom.cdx.json
trivy mise scan
```

#### Inspecting Installed Plugin Metadata

List installed plugins or inspect the `mise` plugin's version, summary, and
usage description (from `plugin.yaml`):

```bash
trivy plugin list
trivy plugin info mise
```

#### Running On the Fly (`trivy plugin run`)

Run the plugin directly without keeping a permanent manual step between install
and execution:

```bash
trivy plugin run github.com/WJ44/trivy-plugin-mise scan
trivy plugin run github.com/WJ44/trivy-plugin-mise sbom --output sbom.cdx.json
```

#### Running via `TRIVY_RUN_AS_PLUGIN`

Set `TRIVY_RUN_AS_PLUGIN=mise` to make `trivy` dispatch directly to the `mise`
plugin without repeating `mise` on the command line:

```bash
TRIVY_RUN_AS_PLUGIN=mise trivy scan
TRIVY_RUN_AS_PLUGIN=mise trivy sbom --output sbom.cdx.json
```

#### Upgrading & Uninstalling

Upgrade the installed `mise` plugin to the latest release or uninstall it:

```bash
trivy plugin upgrade mise
trivy plugin uninstall mise
```

#### Plugin Cache Directory (`XDG_DATA_HOME`)

Trivy resolves the plugin installation directory as follows:

- **`$XDG_DATA_HOME/.trivy/plugins`** when `XDG_DATA_HOME` is set (for example,
  `$XDG_DATA_HOME/.trivy/plugins/mise/`).
- **`~/.trivy/plugins`** when `XDG_DATA_HOME` is unset.

Setting `XDG_DATA_HOME` to a temporary or project-scoped directory allows CI
jobs and integration tests to install and run `trivy-plugin-mise` without
mutating the user's global `~/.trivy/plugins` directory.

### Method 2: Standalone Binary via `mise` (`trivy-plugin-mise ...`)

Pin `trivy` and `trivy-plugin-mise` in your repository's `mise.toml` using the
`github:` backend:

```toml
[tools]
trivy = "0.76.2"
"github:WJ44/trivy-plugin-mise" = "1.1.0"
```

Install the tools (and optionally run `mise lock` to lock platform URLs and
checksums in `mise.lock`):

```bash
mise install
```

Invoke `trivy-plugin-mise` directly (or via `mise x --` / `mise run`):

```bash
trivy-plugin-mise --help
trivy-plugin-mise sbom --output sbom.cdx.json
trivy-plugin-mise scan
```

---

## CLI Reference

All subcommands accept identically structured flags whether invoked as
`trivy mise <subcommand>` or `trivy-plugin-mise <subcommand>`.

### `sbom`

Generate a CycloneDX 1.7 JSON SBOM including `mise`-managed tools and transitive
sidecar dependencies.

```text
trivy-plugin-mise sbom [OPTIONS] [PATH]
trivy mise sbom [OPTIONS] [PATH]
```

| Argument / Flag       | Default       | Description                                                                                                                                                                                            |
| --------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `[PATH]`              | `.`           | Target repository directory containing `mise.lock` and/or `.mise/locks/`.                                                                                                                              |
| `--mode <MODE>`       | `formulation` | Output mode: `formulation` (product SBOM via `trivy fs` + `mise` toolchain under `formulation[]`) or `components` (standalone `mise` toolchain SBOM under top-level `components[]`).                   |
| `-o, --output <PATH>` | stdout        | Write pretty-printed CycloneDX 1.7 JSON to `<PATH>` (creating parent directories if needed) instead of standard output.                                                                                |
| `--platform <TRIPLE>` | _(none)_      | Target platform triple (for example `linux-x64`, `linux-arm64`, `macos-arm64`, `windows-x64`). Selects platform-specific component `hashes[]`, download URLs, and provenance; when omitted, emits all. |
| `--no-verify-digests` | `false`       | Skip SHA-256 digest verification for `.mise/locks/` sidecar lockfiles against `mise.lock` `digest` entries. By default, digest mismatches abort SBOM generation with a non-zero exit code.             |

### `scan`

Scan the repository and `mise`-managed tool dependencies for vulnerabilities and
license compliance.

```text
trivy-plugin-mise scan [OPTIONS] [TARGET]
trivy mise scan [OPTIONS] [TARGET]
```

| Argument / Flag         | Default                      | Description                                                                                                                                                                       |
| ----------------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[TARGET]`              | `.`                          | Target repository directory to scan.                                                                                                                                              |
| `--config <PATH>`       | `<TARGET>/trivy.yaml`        | Path to a Trivy configuration file. If omitted and `<TARGET>/trivy.yaml` exists as a file, it is passed automatically to every Trivy invocation.                                  |
| `--ignorefile <PATH>`   | `<TARGET>/.trivyignore.yaml` | Path to a Trivy ignore file. If omitted and `<TARGET>/.trivyignore.yaml` exists as a file, it is passed automatically to every Trivy invocation.                                  |
| `--severity <SEVERITY>` | _(from config / Trivy)_      | Comma-separated list of severities to include (for example `HIGH,CRITICAL` or `UNKNOWN,MEDIUM,HIGH,CRITICAL`), forwarded as `--severity` to Trivy.                                |
| `--exit-code <CODE>`    | `1`                          | Exit code returned when unaccepted vulnerability or license findings are detected in any enabled scan tier.                                                                       |
| `--repo-only`           | `false`                      | Run only Tier 1 (repository scan) and skip `.mise/locks/` toolchain sidecars and `--include-gobinary`. Conflicts with `--toolchain-only`.                                         |
| `--toolchain-only`      | `false`                      | Run only Tier 2 (`.mise/locks/` sidecar lockfiles, plus Tier 3 if `--include-gobinary` is also set) and skip the Tier 1 repository root scan. Conflicts with `--repo-only`.       |
| `--include-gobinary`    | `false`                      | Enable Tier 3: query active `mise` tool install directories via `mise ls --json` and scan each directory with `trivy rootfs --scanners vuln` (ignored when `--repo-only` is set). |

### `version` / `--version`

Print the CLI package name and version (`trivy-plugin-mise <version>`) and exit
with code `0`:

```bash
trivy-plugin-mise version
trivy-plugin-mise --version
trivy mise version
```

---

## `mise.lock` & Sidecar Lockfiles (`.mise/locks/`)

### `mise.lock` Parsing (`lockfile_version = 3`)

`trivy-plugin-mise` parses `<PATH>/mise.lock` when present. It requires
`lockfile_version = 3` (returning an error on unsupported lockfile versions) and
supports both single-table (`[tools.<name>]`) and array-of-tables
(`[[tools.<name>]]`) tool definitions.

Each locked tool entry is normalized into a CycloneDX `application` component
using its effective backend (`tool.backend` when present, or the
`[tools.<name>]` key when `backend` is omitted).

### Backend-to-PURL Mappings

Locked tools are mapped to standard
[Package URLs (PURLs)](https://github.com/package-url/purl-spec) as follows:

| `mise.lock` Backend Prefix                 | Generated Package URL (PURL)                                    | Example (`backend` + `version`)                                                          |
| ------------------------------------------ | --------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `npm:<package>` (unscoped)                 | `pkg:npm/<lowercased-name>@<version>`                           | `npm:prettier` (`3.5.3`) -> `pkg:npm/prettier@3.5.3`                                     |
| `npm:@<scope>/<name>` (scoped)             | `pkg:npm/%40<lowercased-scope>/<lowercased-name>@<version>`     | `npm:@commitlint/cli` (`19.8.0`) -> `pkg:npm/%40commitlint/cli@19.8.0`                   |
| `pipx:<package>` or `python:<package>`     | `pkg:pypi/<normalized-name>@<version>` (lowercased, `_` -> `-`) | `pipx:pre_commit` (`4.2.0`) -> `pkg:pypi/pre-commit@4.2.0`                               |
| `cargo:<crate>`                            | `pkg:cargo/<crate>@<version>`                                   | `cargo:cargo-audit` (`0.21.2`) -> `pkg:cargo/cargo-audit@0.21.2`                         |
| `aqua:<owner>/<repo>[/<subpath>]`          | `pkg:github/<owner>/<repo>@<version>[#<subpath>]` (lowercased)  | `aqua:aquasecurity/trivy` (`0.76.2`) -> `pkg:github/aquasecurity/trivy@0.76.2`           |
| `github:<owner>/<repo>[/<subpath>]`        | `pkg:github/<owner>/<repo>@<version>[#<subpath>]` (lowercased)  | `github:WJ44/trivy-plugin-mise` (`1.1.0`) -> `pkg:github/wj44/trivy-plugin-mise@1.1.0`   |
| `ubi:<owner>/<repo>[/<subpath>]`           | `pkg:github/<owner>/<repo>@<version>[#<subpath>]` (lowercased)  | `ubi:BurntSushi/ripgrep` (`14.1.1`) -> `pkg:github/burntsushi/ripgrep@14.1.1`            |
| `ubi:https://...` (direct URL)             | `pkg:generic/<tool-name>@<version>?download_url=<url>`          | `ubi:https://example.com/tool.tar.gz` -> `pkg:generic/tool@1.0.0?download_url=...`       |
| `go:<module-path>`                         | `pkg:golang/[<namespace>/]<name>@<version>`                     | `go:mvdan.cc/sh/v3/cmd/shfmt` (`3.11.0`) -> `pkg:golang/mvdan.cc/sh/v3/cmd/shfmt@3.11.0` |
| `core:<name>` or any other / bare tool key | `pkg:generic/<name>@<version>[?download_url=<platform-url>]`    | `core:node` (`22.14.0`) -> `pkg:generic/node@22.14.0`                                    |

### Platform Checksums, Download URLs & Provenance Properties

Each normalized `mise.lock` component sets `bom-ref` to its PURL and records
`mise:*` properties and references from `[tools.<name>.platforms.<triple>]`:

- **Base properties (always emitted):**
  - `mise:tool`: Tool name key in `mise.lock`.
  - `mise:backend`: Effective backend specifier (e.g. `aqua:aquasecurity/trivy`
    or `npm:@commitlint/cli`).
  - `mise:<sidecar>:path` and `mise:<sidecar>:digest`: Emitted when a tool entry
    references a sidecar lockfile table (for example
    `aube = { path = ".mise/locks/...", digest = "sha256:..." }`).
- **When `--platform <TRIPLE>` is specified (and present on the tool):**
  - Top-level component `hashes[]` is populated from
    `platforms.<TRIPLE>.checksum` (supporting `sha256:`, `sha384:`, `sha512:`,
    `sha1:`, and `md5:` prefixes mapped to CycloneDX `SHA-256`, `SHA-384`,
    `SHA-512`, `SHA-1`, and `MD5`).
  - `mise:provenance` is set if `platforms.<TRIPLE>.provenance` is present (e.g.
    `github-attestations`, `cosign`, `slsa`).
  - Platform properties `mise:platform:<TRIPLE>:checksum`,
    `mise:platform:<TRIPLE>:url`, `mise:platform:<TRIPLE>:provenance`, and
    `mise:platform:<TRIPLE>:checksum_source` are emitted for `<TRIPLE>`, along
    with a `distribution` entry in `externalReferences[]` when `url` is present.
  - For `pkg:generic` tools (and `ubi:` URL backends), `?download_url=<url>` is
    appended to the PURL when `platforms.<TRIPLE>.url` is available.
- **When `--platform` is omitted (default):**
  - Top-level `hashes[]` is left empty because a single component entry
    represents all locked platforms.
  - `mise:platform:<plat>:*` properties and `distribution` `externalReferences`
    are emitted for **every** platform present in `mise.lock`.
  - Top-level `mise:provenance` is emitted when all provenance-bearing platforms
    on the tool agree on the same provenance type.

### `.mise/locks/<tool>/<version>/` Sidecars & Digest Verification

When `mise` installs ecosystem-backed tools (such as `npm:` packages locked via
`aube` or Python tools locked via `uv`), transitive dependency lockfiles can be
stored under `.mise/locks/<tool>/<version>/`.

1. **Discovery & Path Validation:**
   - `trivy-plugin-mise` first reads sidecar references declared on tools in
     `mise.lock` (e.g.
     `aube = { path = ".mise/locks/npm-prettier/3.5.3", digest = "sha256:..." }`),
     and then scans `.mise/locks/<tool>/<version>/` for any additional sidecar
     directories.
   - All sidecar paths are validated to ensure they remain strictly inside the
     target project root (rejecting absolute paths, `..` traversal, or symlinks
     escaping the workspace).
2. **SHA-256 Digest Verification:**
   - When a sidecar entry in `mise.lock` specifies `digest = "sha256:<64-hex>"`,
     `trivy-plugin-mise` computes the SHA-256 digest of the sidecar lockfile
     (`aube-lock.yaml` for `aube`, `uv.lock` for `uv`, or the first present file
     among `aube-lock.yaml`, `uv.lock`, `pnpm-lock.yaml`, `package-lock.json`,
     and `Cargo.lock`) and verifies that it matches.
   - Digest verification is active by default in both `sbom` and `scan`. In
     `sbom`, it can be bypassed with `--no-verify-digests`.
3. **Ephemeral Workspace Materialization (`aube-lock.yaml` ->
   `pnpm-lock.yaml`):**
   - Because Trivy's filesystem analyzer recognizes standard ecosystem filenames
     (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `uv.lock`,
     `Cargo.lock`) rather than `aube-lock.yaml`, `trivy-plugin-mise` creates an
     ephemeral temporary directory (`trivy-plugin-mise-*`) and copies recognized
     manifests and lockfiles (`package.json`, `pnpm-lock.yaml`,
     `package-lock.json`, `yarn.lock`, `uv.lock`, `pyproject.toml`,
     `Cargo.lock`, `Cargo.toml`) into it.
   - Any `aube-lock.yaml` file (which uses standard pnpm lockfile syntax) is
     materialized as `pnpm-lock.yaml` inside the temporary workspace so
     `trivy fs` parses every transitive npm dependency and dependency edge
     without modifying `.mise/locks/` in the working tree.

---

## SBOM Generation Modes

`trivy-plugin-mise sbom` always outputs a CycloneDX `1.7` document
(`"$schema": "http://cyclonedx.org/schema/bom-1.7.schema.json"`,
`"specVersion": "1.7"`). It deduplicates components across `mise.lock` and
`.mise/locks/` sidecars by PURL (preserving `mise.lock` hashes and `mise:*`
properties while merging license metadata discovered by Trivy from sidecars) and
remaps transitive dependency edges under a `"mise-toolchain"` root reference.

### `--mode formulation` (Default)

Use `--mode formulation` when generating the primary SBOM for a repository or
deliverable product:

1. Runs `trivy fs --format cyclonedx --quiet --skip-dirs .mise/locks <PATH>` to
   produce the base product SBOM (whose top-level `components[]` contains only
   shipped runtime/application dependencies).
2. Upgrades the document schema to CycloneDX `1.7` and appends a formula entry
   (`"bom-ref": "mise-toolchain"`) to top-level **`formulation[]`** containing:
   - `components`: All normalized `mise.lock` tools and deduplicated transitive
     `.mise/locks/` sidecar components.
   - `workflows`: A standardized workflow descriptor
     (`"bom-ref": "workflow:mise-toolchain"`, `"uid": "mise-toolchain"`,
     `"taskTypes": ["build", "lint", "test", "scan"]`).
3. Merges the `"mise-toolchain"` dependency graph into top-level
   `dependencies[]` without adding toolchain packages to the shipped application
   root component's `dependsOn` list.

**Why `formulation[]`?** In CycloneDX 1.7 (and supply-chain standards such as
CISA SBOM Minimum Elements and the EU Cyber Resilience Act), build and
development tools belong in the Manufacturing BOM (`formulation[]`) rather than
shipped `components[]`. Separating them prevents downstream consumers and
license scanners from mistaking build-time linters or formatters (e.g.
GPL-licensed CLI tools) for code linked into the distributed product.

### `--mode components`

Use `--mode components` when you want a standalone SBOM of the `mise` toolchain
itself (for example, to ingest into an external SBOM analysis tool that only
inspects top-level `components[]`):

- Skips scanning the repository root for product dependencies.
- Emits a standalone CycloneDX 1.7 document whose `metadata.component` is
  `{ "bom-ref": "mise-toolchain", "type": "application", "name": "<dir-name>" }`
  and whose top-level `components[]` and `dependencies[]` contain the
  `mise.lock` tools and `.mise/locks/` transitive packages directly.

---

## Scanning Tiers & Configuration

`trivy-plugin-mise scan` runs up to three distinct scanning tiers and prints a
summary line (`==> Scan summary: repo=..., toolchain=..., gobinary=... -> ...`).
If any enabled tier detects unaccepted findings, `scan` exits with `--exit-code`
(default `1`).

### The Three Scan Tiers

| Tier               | Default State | Underlying Trivy Command                                                    | Scanners          | Purpose                                                                                                                                                                                                                     |
| ------------------ | ------------- | --------------------------------------------------------------------------- | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1. `repo`**      | **Enabled**   | `trivy fs --scanners vuln,license --skip-dirs .mise/locks <TARGET>`         | `vuln`, `license` | Scans shipped repository manifests and lockfiles for both CVEs and license policy compliance. Skipped when `--toolchain-only` is passed.                                                                                    |
| **2. `toolchain`** | **Enabled**   | `trivy fs --scanners vuln <materialized-sidecar-workspace>`                 | `vuln` only       | Verifies SHA-256 digests, materializes `.mise/locks/` (`aube-lock.yaml` -> `pnpm-lock.yaml`, `uv.lock`, etc.), and scans tool dependencies for CVEs. Skipped if no sidecars exist or `--repo-only` is passed.               |
| **3. `gobinary`**  | **Opt-in**    | `trivy rootfs --scanners vuln <active-mise-install-path>` (per active tool) | `vuln` only       | Enabled with `--include-gobinary` (unless `--repo-only` is set). Runs `mise ls --json` in `<TARGET>` and scans every installed, active tool directory (catching Go stdlib/module CVEs compiled into prebuilt CLI binaries). |

### Automatic Discovery of `trivy.yaml` and `.trivyignore.yaml`

Unless overridden via `--config <PATH>` or `--ignorefile <PATH>`,
`trivy-plugin-mise scan` checks for `<TARGET>/trivy.yaml` and
`<TARGET>/.trivyignore.yaml` and forwards `--config` and `--ignorefile` to every
Trivy invocation across all active tiers.

#### Example `trivy.yaml`

```yaml
exit-code: 1
ignorefile: .trivyignore.yaml

severity:
  - UNKNOWN
  - MEDIUM
  - HIGH
  - CRITICAL

scan:
  scanners:
    - vuln
    - license
```

#### Example `.trivyignore.yaml`

```yaml
vulnerabilities:
  - id: CVE-2026-1234
    statement: "Dev-only CLI dependency; not reachable in non-interactive runs."
    expired_at: 2026-12-31
    purls:
      - "pkg:npm/example-pkg"

licenses:
  - id: MPL-2.0
    statement: "Weak copyleft license approved for file-level use."
```

> **Note:** Keep `expired_at` dates unquoted (`YYYY-MM-DD`) in
> `.trivyignore.yaml`, as Trivy's YAML parser rejects quoted date-only strings.

---

## `mise.toml` Tasks & CI Integration

### `mise.toml` Task Recipes

Add `sbom` and `scan` tasks to your repository's `mise.toml`:

```toml
[tools]
trivy = "0.76.2"
"github:WJ44/trivy-plugin-mise" = "1.1.0"

[tasks.sbom]
description = "Generate CycloneDX SBOM (sbom.cdx.json)"
run = "trivy-plugin-mise sbom --output sbom.cdx.json"

[tasks.scan]
description = "Scan repository and mise toolchain for CVEs and license compliance"
run = "trivy-plugin-mise scan"
```

Then run locally:

```bash
mise run sbom
mise run scan
```

### GitHub Actions Workflow Example

Below is a copy-pasteable GitHub Actions job that installs `mise` tools,
generates `sbom.cdx.json`, uploads it as a workflow artifact, and gates CI on
`mise run scan` (including a daily scheduled run for newly disclosed CVEs):

```yaml
name: Security Scan

on:
  push:
    branches:
      - main
  pull_request:
  schedule:
    - cron: "0 6 * * *"

jobs:
  scan:
    name: SBOM and security scan
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install mise tools
        uses: jdx/mise-action@v2
        with:
          cache: true

      - name: Generate CycloneDX SBOM
        run: mise run sbom

      - name: Upload SBOM artifact
        uses: actions/upload-artifact@v4
        with:
          name: sbom.cdx.json
          path: sbom.cdx.json

      - name: Vulnerability and license scan
        run: mise run scan
```
