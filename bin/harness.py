#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""CLI for initializing, updating, rendering, and releasing downstream harness projects."""

from __future__ import annotations

import argparse
import filecmp
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path

HARNESS_VERSION = "1.1.0"  # x-release-please-version
DEFAULT_UPSTREAM = "https://github.com/WJ44/harness.git"
DEFAULT_REF = "main"
HARNESS_META_FILE = ".harness.json"
VALID_MODES = ("normal", "release")

RELEASE_GITIGNORE_START = "# --- harness:release:start ---"
RELEASE_GITIGNORE_END = "# --- harness:release:end ---"

# Paths excluded from git (kept locally on disk) in release mode.
RELEASE_EXCLUDED_PATHS = (
    "AGENTS.md",
    "AGENTS.template.md",
    "CLAUDE.md",
    "README.template.md",
    "templates/",
    "agent-context/harness/",
    "decisions/harness/",
    "docs/harness/",
    ".agents/",
    ".claude/",
    ".mcp.json",
    "hooks/",
    "shim/",
    ".github/ISSUE_TEMPLATE/",
    ".github/PULL_REQUEST_TEMPLATE/",
    ".github/labels.json",
    "bin/harness.py",
    "bin/sync-labels",
    "bin/wait-pr-checks",
    "setup.sh",
    "setup-system.sh",
    "setup-user.sh",
    ".harness.json",
)

# Directories completely owned by the harness (mirrored on init and update).
HARNESS_OWNED_DIRS = (
    "decisions/harness",
    "agent-context/harness",
    "docs/harness",
    ".github/ISSUE_TEMPLATE",
    ".github/PULL_REQUEST_TEMPLATE",
    "hooks",
    "shim",
    "bin",
    "templates",
    ".mise",
)

# Individual files owned by the harness (copied on init, updated on update).
HARNESS_OWNED_FILES = (
    "AGENTS.template.md",
    "README.template.md",
    "CLAUDE.md",
    ".agents/hooks.json",
    ".agents/plugins/context7/mcp_config.json",
    ".agents/plugins/context7/plugin.json",
    ".claude/settings.json",
    ".commitlintrc.json",
    ".editorconfig",
    ".gitattributes",
    ".github/labels.json",
    ".github/workflows/ci.yml",
    ".github/workflows/release-please.yml",
    ".gitignore",
    ".lefthookrc",
    ".mcp.json",
    ".prettierignore",
    ".prettierrc.json",
    ".rumdl.toml",
    "lefthook.yml",
    "mise.lock",
    "mise.toml",
    "renovate.json",
    "setup-system.sh",
    "setup-user.sh",
    "setup.sh",
    "trivy.yaml",
)

# Release-excluded subset used when hydrating an existing release-mode clone.
RELEASE_HYDRATE_DIRS = (
    "decisions/harness",
    "agent-context/harness",
    "docs/harness",
    ".github/ISSUE_TEMPLATE",
    ".github/PULL_REQUEST_TEMPLATE",
    "hooks",
    "shim",
    "templates",
)

RELEASE_HYDRATE_FILES = (
    "AGENTS.template.md",
    "CLAUDE.md",
    "README.template.md",
    ".agents/hooks.json",
    ".agents/plugins/context7/mcp_config.json",
    ".agents/plugins/context7/plugin.json",
    ".claude/settings.json",
    ".github/labels.json",
    ".mcp.json",
    "bin/harness.py",
    "bin/sync-labels",
    "bin/wait-pr-checks",
    "setup.sh",
    "setup-system.sh",
    "setup-user.sh",
)

# Shared harness scripts under bin/ that remain tracked in git in release mode.
SHARED_BIN_FILES = (
    "bin/mise-sbom",
    "bin/validate-decisions.py",
    "bin/validate-trivyignore.yq",
)

# Files initialized from harness on init, then owned by the downstream project.
PROJECT_SEED_FILES = (".trivyignore.yaml",)

RENDER_TARGETS = (
    ("AGENTS.template.md", "agent-context/AGENTS.project.md", "AGENTS.md"),
    ("README.template.md", "agent-context/README.project.md", "README.md"),
)

SECTION_RE = re.compile(
    r"<!--\s*project:(?P<name>[a-zA-Z0-9_-]+):start\s*-->(?P<body>.*?)<!--\s*project:(?P=name):end\s*-->",
    re.DOTALL,
)
NORMAL_ONLY_RE = re.compile(
    r"<!--\s*harness:normal-only:start\s*-->(?P<body>.*?)<!--\s*harness:normal-only:end\s*-->",
    re.DOTALL,
)
PLACEHOLDER_LINE_RE = re.compile(
    r"(?m)^[ \t]*\{\{project:(?P<name>[a-zA-Z0-9_-]+)\}\}[ \t]*\n?"
)
PLACEHOLDER_INLINE_RE = re.compile(r"\{\{project:(?P<name>[a-zA-Z0-9_-]+)\}\}")

RELEASE_GITIGNORE_BLOCK_RE = re.compile(
    rf"(?ms)^{re.escape(RELEASE_GITIGNORE_START)}\n.*?^{re.escape(RELEASE_GITIGNORE_END)}$"
)


def extract_project_sections(project_text: str) -> dict[str, str]:
    """Extract named `<!-- project:<name>:start/end -->` blocks from markdown."""
    sections: dict[str, str] = {}
    for match in SECTION_RE.finditer(project_text):
        name = match.group("name")
        body = match.group("body").strip("\r\n")
        sections[name] = body
    return sections


def strip_normal_only_sections(text: str, mode: str) -> str:
    """Strip `<!-- harness:normal-only:start/end -->` blocks in release mode; keep markers in normal mode."""
    if mode != "release" or not NORMAL_ONLY_RE.search(text):
        return text
    stripped = NORMAL_ONLY_RE.sub("", text)
    stripped = re.sub(r"\n{3,}", "\n\n", stripped).strip("\r\n")
    return f"{stripped}\n" if stripped else ""


def render_template(
    template_text: str,
    project_text: str,
    mode: str = "normal",
) -> str:
    """Substitute project blocks into a template and normalize blank lines."""
    prepared_template = strip_normal_only_sections(template_text, mode=mode)

    sections = extract_project_sections(project_text)

    def replace_line(match: re.Match[str]) -> str:
        name = match.group("name")
        body = sections.get(name, "").strip("\r\n")
        if not body:
            return ""
        return f"{body}\n"

    rendered = PLACEHOLDER_LINE_RE.sub(replace_line, prepared_template)
    rendered = PLACEHOLDER_INLINE_RE.sub(
        lambda m: sections.get(m.group("name"), "").strip("\r\n"),
        rendered,
    )
    rendered = re.sub(r"\n{3,}", "\n\n", rendered).strip("\r\n")
    return f"{rendered}\n" if rendered else ""


def build_release_gitignore_block() -> str:
    """Build the managed release-mode `.gitignore` exclusion block."""
    entries = [f"/{path}" for path in RELEASE_EXCLUDED_PATHS]
    return "\n".join([RELEASE_GITIGNORE_START, *entries, RELEASE_GITIGNORE_END])


def has_release_gitignore_block(root: Path) -> bool:
    """Return True if `.gitignore` under `root` contains the managed release block."""
    gitignore_path = root / ".gitignore"
    if not gitignore_path.is_file():
        return False
    text = gitignore_path.read_text(encoding="utf-8")
    return RELEASE_GITIGNORE_START in text and RELEASE_GITIGNORE_END in text


def apply_release_gitignore(text: str) -> str:
    """Insert or update the managed release-ignore block in `.gitignore` text."""
    block = build_release_gitignore_block()
    if RELEASE_GITIGNORE_BLOCK_RE.search(text):
        updated = RELEASE_GITIGNORE_BLOCK_RE.sub(lambda _: block, text, count=1)
        return updated.rstrip("\n") + "\n"
    base = text.rstrip("\n")
    if not base:
        return f"{block}\n"
    return f"{base}\n\n{block}\n"


def ensure_release_gitignore(
    target_root: Path, src_root: Path | None = None, dry_run: bool = False
) -> str | None:
    """Ensure `.gitignore` under `target_root` contains the release-ignore block."""
    dst = target_root / ".gitignore"
    if dst.is_file():
        current = dst.read_text(encoding="utf-8")
        action: str | None = "update"
    else:
        src_gitignore = (src_root / ".gitignore") if src_root else None
        current = (
            src_gitignore.read_text(encoding="utf-8")
            if src_gitignore and src_gitignore.is_file()
            else ""
        )
        action = "add"

    updated = apply_release_gitignore(current)
    if dst.is_file() and updated == current:
        return None
    if not dry_run:
        dst.write_text(updated, encoding="utf-8")
    return action


def resolve_effective_mode(root: Path, requested_release: bool = False) -> str:
    """Determine active harness mode, enforcing one-way transition into release mode."""
    if requested_release or has_release_gitignore_block(root):
        return "release"
    if (root / HARNESS_META_FILE).is_file():
        return str(read_harness_meta(root)["mode"])
    return "normal"


def render_root(
    root: Path,
    check: bool = False,
    mode: str | None = None,
    only_targets: set[str] | None = None,
) -> bool:
    """Render AGENTS.md and README.md under `root`. Returns True if all up-to-date/ok."""
    effective_mode = mode if mode is not None else resolve_effective_mode(root)
    all_ok = True
    for template_name, project_name, output_name in RENDER_TARGETS:
        if only_targets is not None and output_name not in only_targets:
            continue
        template_path = root / template_name
        project_path = root / project_name
        output_path = root / output_name

        if not template_path.is_file():
            raise FileNotFoundError(f"Missing template file: {template_path}")
        if not project_path.is_file():
            raise FileNotFoundError(f"Missing project file: {project_path}")

        rendered = render_template(
            template_path.read_text(encoding="utf-8"),
            project_path.read_text(encoding="utf-8"),
            mode=effective_mode,
        )
        current = (
            output_path.read_text(encoding="utf-8") if output_path.is_file() else None
        )

        if check:
            if current != rendered:
                print(f"Out of date: {output_name}", file=sys.stderr)
                all_ok = False
        else:
            if current != rendered:
                output_path.write_text(rendered, encoding="utf-8")
                print(f"Rendered {output_name}")
            else:
                print(f"Up to date: {output_name}")

    return all_ok


def render_starter_file(
    templates_dir: Path,
    filename: str,
    name: str,
    description: str,
    mode: str = "normal",
) -> str:
    """Load a starter template file and substitute project name/description."""
    template_path = templates_dir / filename
    if not template_path.is_file():
        raise FileNotFoundError(f"Missing starter template: {template_path}")
    content = strip_normal_only_sections(
        template_path.read_text(encoding="utf-8"), mode=mode
    )
    return content.replace("{{project:name}}", name).replace(
        "{{project:description}}", description
    )


SEMVER_RE = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)"
    r"(?:-(?P<pre>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+(?P<build>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)
RELEASE_TAG_RE = re.compile(
    r"^v(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)$"
)
HARNESS_VERSION_ASSIGN_RE = re.compile(
    r'^HARNESS_VERSION\s*=\s*["\']([^"\']+)["\']',
    re.MULTILINE,
)
CHANGELOG_VERSION_HEADING_RE = re.compile(
    r"^(?P<hashes>#{2,3})\s+\[?v?(?P<version>(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)\]?(?:\([^)]*\))?(?:\s+.*)?$"
)
FENCE_RE = re.compile(r"^\s{0,3}(?P<fence>`{3,}|~{3,})")


@dataclass(frozen=True)
class SemVer:
    """Semantic Version 2.0.0 value object supporting precedence comparisons."""

    major: int
    minor: int
    patch: int
    pre: tuple[str, ...] | None = None
    raw: str = field(default="", compare=False)

    def _sort_key(
        self,
    ) -> tuple[int, int, int, int, tuple[tuple[int, int | str], ...]]:
        if self.pre is None:
            return (self.major, self.minor, self.patch, 1, ())
        parts = tuple(
            (0, int(ident)) if ident.isdigit() else (1, ident) for ident in self.pre
        )
        return (self.major, self.minor, self.patch, 0, parts)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return self._sort_key() == other._sort_key()

    def __lt__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return self._sort_key() < other._sort_key()

    def __le__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return self._sort_key() <= other._sort_key()

    def __gt__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return self._sort_key() > other._sort_key()

    def __ge__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return self._sort_key() >= other._sort_key()

    def __hash__(self) -> int:
        return hash(self._sort_key())

    def __str__(self) -> str:
        if self.raw:
            return self.raw
        base = f"{self.major}.{self.minor}.{self.patch}"
        return f"{base}-{'.'.join(self.pre)}" if self.pre else base


def parse_semver(version: str) -> SemVer:
    """Parse and validate a SemVer 2.0.0 version string."""
    match = SEMVER_RE.match(version.strip())
    if not match:
        raise RuntimeError(f"Invalid semantic version: '{version}'")
    pre_raw = match.group("pre")
    pre = tuple(pre_raw.split(".")) if pre_raw else None
    return SemVer(
        major=int(match.group("major")),
        minor=int(match.group("minor")),
        patch=int(match.group("patch")),
        pre=pre,
        raw=version.strip(),
    )


def read_harness_version(root: Path) -> str:
    """Read and validate `HARNESS_VERSION` from `<root>/bin/harness.py`."""
    harness_script = root / "bin" / "harness.py"
    if not harness_script.is_file():
        raise RuntimeError(
            f"Missing {harness_script}; cannot determine upstream harness version."
        )
    text = harness_script.read_text(encoding="utf-8")
    match = HARNESS_VERSION_ASSIGN_RE.search(text)
    if not match:
        raise RuntimeError(
            f"Missing HARNESS_VERSION in {harness_script}; target may predate SemVer support."
        )
    version = match.group(1).strip()
    parse_semver(version)
    return version


def parse_latest_semver_tag(tag_names: Iterable[str]) -> str | None:
    """Return the highest `vX.Y.Z` SemVer tag from `tag_names`, or None if none match."""
    best_tuple: tuple[int, int, int] | None = None
    best_tag: str | None = None
    for raw in tag_names:
        tag = raw.strip()
        match = RELEASE_TAG_RE.match(tag)
        if not match:
            continue
        ver_tuple = (
            int(match.group("major")),
            int(match.group("minor")),
            int(match.group("patch")),
        )
        if best_tuple is None or ver_tuple > best_tuple:
            best_tuple = ver_tuple
            best_tag = tag
    return best_tag


def update_fence_state(
    line: str, fence_char: str | None, fence_len: int
) -> tuple[bool, str | None, int]:
    """Track Markdown fenced code blocks; return (is_fence_line, fence_char, fence_len)."""
    match = FENCE_RE.match(line)
    if not match:
        return False, fence_char, fence_len

    marker = match.group("fence")
    rest = line[match.end() :]
    if fence_char is None:
        if marker[0] == "`" and "`" in rest:
            return False, None, 0
        return True, marker[0], len(marker)

    if marker[0] == fence_char and len(marker) >= fence_len and not rest.strip():
        return True, None, 0

    return False, fence_char, fence_len


def extract_changelog_notes(
    changelog_path: Path, current: SemVer, target: SemVer
) -> str:
    """Extract release notes for versions `(current, target]` from `changelog_path`."""
    if not changelog_path.is_file() or target <= current:
        return ""
    lines = changelog_path.read_text(encoding="utf-8").splitlines()
    sections: list[str] = []
    current_lines: list[str] | None = None
    fence_char: str | None = None
    fence_len = 0

    def flush() -> None:
        nonlocal current_lines
        if current_lines:
            body = "\n".join(current_lines).strip()
            if body:
                sections.append(body)
        current_lines = None

    for line in lines:
        is_fence, fence_char, fence_len = update_fence_state(
            line, fence_char, fence_len
        )
        if is_fence or fence_char is not None:
            if current_lines is not None:
                current_lines.append(line)
            continue

        match = CHANGELOG_VERSION_HEADING_RE.match(line)
        if match:
            flush()
            try:
                ver = parse_semver(match.group("version"))
            except RuntimeError:
                continue
            if current < ver <= target:
                current_lines = [line]
            continue
        if current_lines is not None:
            if re.match(r"^#{1,2}\s+", line):
                flush()
            else:
                current_lines.append(line)

    flush()
    return "\n\n".join(sections).strip()


def is_local_source(source: str) -> bool:
    """Return True if `source` points to an existing local directory."""
    return Path(source).expanduser().is_dir()


def is_bare_git_repo(path: Path) -> bool:
    """Return True if `path` looks like a bare git repository."""
    return (
        (path / "HEAD").is_file()
        and (path / "objects").is_dir()
        and not (path / ".git").exists()
    )


def is_git_repo(path: Path) -> bool:
    """Return True if `path` is a normal or bare git repository."""
    return (path / ".git").exists() or is_bare_git_repo(path)


def get_git_commit(repo_dir: Path) -> str | None:
    """Return HEAD commit SHA if `repo_dir` is a git repository with commits, else None."""
    if not is_git_repo(repo_dir) or not shutil.which("git"):
        return None
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "rev-parse", "--verify", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return result.stdout.strip()
    return None


def resolve_default_ref(source: str) -> str:
    """Resolve the latest `vX.Y.Z` SemVer tag on `source`, falling back to `DEFAULT_REF`."""
    if is_local_source(source):
        src_path = Path(source).expanduser().resolve()
        if is_git_repo(src_path) and shutil.which("git"):
            res = subprocess.run(
                ["git", "-C", str(src_path), "tag", "--list"],
                capture_output=True,
                text=True,
                check=False,
            )
            if res.returncode != 0:
                raise RuntimeError(
                    f"Failed to list tags in '{src_path}': {res.stderr.strip()}"
                )
            return parse_latest_semver_tag(res.stdout.splitlines()) or DEFAULT_REF
        return DEFAULT_REF

    if not shutil.which("git"):
        raise RuntimeError("git is required to query upstream release tags.")
    res = subprocess.run(
        ["git", "ls-remote", "--tags", "--refs", source],
        capture_output=True,
        text=True,
        check=False,
    )
    if res.returncode != 0:
        raise RuntimeError(
            f"Failed to query upstream tags from '{source}': {res.stderr.strip()}"
        )
    tags = [
        line.split("\t", 1)[1].removeprefix("refs/tags/")
        for line in res.stdout.splitlines()
        if "\trefs/tags/" in line
    ]
    return parse_latest_semver_tag(tags) or DEFAULT_REF


def resolve_local_git_ref(repo_dir: Path, ref: str) -> str:
    """Resolve a branch, tag, or revision in a local git repo to a commit SHA."""
    if ref == "HEAD":
        candidates: tuple[str, ...] = ("HEAD^{commit}",)
    else:
        candidates = (
            f"refs/tags/{ref}^{{commit}}",
            f"refs/heads/{ref}^{{commit}}",
            f"refs/remotes/origin/{ref}^{{commit}}",
            f"{ref}^{{commit}}",
        )
    for candidate in candidates:
        res = subprocess.run(
            ["git", "-C", str(repo_dir), "rev-parse", "--verify", candidate],
            capture_output=True,
            text=True,
            check=False,
        )
        if res.returncode == 0:
            return res.stdout.strip()
    raise RuntimeError(f"Failed to resolve ref '{ref}' in local repo '{repo_dir}'.")


def resolve_upstream(
    source: str, ref: str, tmpdir: Path
) -> tuple[Path, str, str | None]:
    """Resolve upstream harness directory and return (path, upstream_url, commit)."""
    if is_local_source(source):
        src_path = Path(source).expanduser().resolve()
        if is_git_repo(src_path) and get_git_commit(src_path) is not None:
            commit = resolve_local_git_ref(src_path, ref)
            export_dir = tmpdir / "upstream"
            export_dir.mkdir(parents=True, exist_ok=True)
            archive = subprocess.run(
                ["git", "-C", str(src_path), "archive", "--format=tar", commit],
                capture_output=True,
                check=False,
            )
            if archive.returncode != 0:
                stderr = archive.stderr.decode("utf-8", errors="replace").strip()
                raise RuntimeError(
                    f"Failed to export '{src_path}' at ref '{ref}': {stderr}"
                )
            with tarfile.open(fileobj=io.BytesIO(archive.stdout), mode="r:") as tf:
                if hasattr(tarfile, "data_filter"):
                    tf.extractall(export_dir, filter="data")
                else:
                    tf.extractall(export_dir)
            return export_dir, DEFAULT_UPSTREAM, commit
        return src_path, DEFAULT_UPSTREAM, get_git_commit(src_path)

    clone_dir = tmpdir / "upstream"
    cmd = ["git", "clone", "--depth", "1", "--branch", ref, source, str(clone_dir)]
    result = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise RuntimeError(
            f"Failed to clone '{source}' (ref '{ref}'): {result.stderr.strip()}"
        )
    commit = get_git_commit(clone_dir)
    return clone_dir, source, commit


def sha256_file(path: Path) -> str:
    """Return the hex SHA-256 digest of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_ignored_sync_path(path: Path) -> bool:
    """Return True for ephemeral Python bytecode artifacts skipped during sync/hashing."""
    return "__pycache__" in path.parts or path.suffix in (".pyc", ".pyo")


def collect_harness_hashes(
    root: Path,
    src_root: Path | None = None,
    allowed_shared_files: set[str] | None = None,
    filter_mise: bool = False,
) -> dict[str, str]:
    """Compute SHA-256 hashes for harness-owned files present under `root`."""
    hashes: dict[str, str] = {}
    for rel_dir in HARNESS_OWNED_DIRS:
        dir_path = root / rel_dir
        if not dir_path.is_dir():
            continue
        for path in sorted(dir_path.rglob("*")):
            if not path.is_file():
                continue
            rel_path = path.relative_to(root)
            if is_ignored_sync_path(rel_path):
                continue
            rel = rel_path.as_posix()
            if rel_dir in ("bin", ".mise"):
                if src_root is not None and not (src_root / rel).is_file():
                    continue
                if (
                    src_root is None
                    and allowed_shared_files is not None
                    and (rel_dir == "bin" or filter_mise)
                    and rel not in allowed_shared_files
                ):
                    continue
            hashes[rel] = sha256_file(path)
    for rel_file in HARNESS_OWNED_FILES:
        file_path = root / rel_file
        if file_path.is_file():
            hashes[rel_file] = sha256_file(file_path)
    return hashes


def write_harness_meta(
    target_root: Path,
    upstream: str,
    ref: str,
    commit: str | None,
    mode: str = "normal",
    files: dict[str, str] | None = None,
    version: str = HARNESS_VERSION,
) -> None:
    """Write .harness.json metadata in the downstream repository."""
    if mode not in VALID_MODES:
        raise RuntimeError(f"Unsupported harness mode: {mode}")
    parse_semver(version)
    meta = {
        "upstream": upstream,
        "ref": ref,
        "commit": commit or "",
        "version": version,
        "mode": mode,
        "files": files if files is not None else collect_harness_hashes(target_root),
    }
    meta_path = target_root / HARNESS_META_FILE
    meta_path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")


def read_harness_meta(target_root: Path) -> dict[str, object]:
    """Read and validate .harness.json if present, returning defaults otherwise."""
    meta_path = target_root / HARNESS_META_FILE
    if not meta_path.is_file():
        return {
            "upstream": DEFAULT_UPSTREAM,
            "ref": DEFAULT_REF,
            "commit": "",
            "version": HARNESS_VERSION,
            "mode": "normal",
            "files": {},
        }
    try:
        data = json.loads(meta_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Invalid JSON in {HARNESS_META_FILE}: {exc}") from exc
    if not isinstance(data, dict):
        raise TypeError(f"Invalid {HARNESS_META_FILE}: expected a JSON object.")
    raw_version = data.get("version")
    if not isinstance(raw_version, str) or not raw_version.strip():
        raise RuntimeError(
            f"Missing or invalid 'version' in {HARNESS_META_FILE}; expected a SemVer string (e.g. '{HARNESS_VERSION}')."
        )
    version = raw_version.strip()
    parse_semver(version)
    mode = str(data.get("mode") or "normal")
    if mode not in VALID_MODES:
        raise RuntimeError(f"Unsupported harness mode in {HARNESS_META_FILE}: {mode}")
    raw_files = data.get("files")
    files = (
        {str(k): str(v) for k, v in raw_files.items()}
        if isinstance(raw_files, dict)
        else {}
    )
    return {
        "upstream": str(data.get("upstream") or DEFAULT_UPSTREAM),
        "ref": str(data.get("ref") or DEFAULT_REF),
        "commit": str(data.get("commit") or ""),
        "version": version,
        "mode": mode,
        "files": files,
    }


def copy_file_if_changed(src: Path, dst: Path, dry_run: bool = False) -> str | None:
    """Copy `src` to `dst` if missing or different. Returns 'add'|'update'|None."""
    if not dst.exists():
        if not dry_run:
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
        return "add"
    # Guard against non-regular files (e.g. directories) before calling filecmp.cmp.
    if not dst.is_file() or not filecmp.cmp(src, dst, shallow=False):
        if not dry_run:
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
        return "update"
    return None


def sync_dir_tree(
    src_root: Path,
    dst_root: Path,
    rel_dir: str,
    dry_run: bool = False,
    preserve_unrecorded: bool = False,
    recorded_files: dict[str, str] | None = None,
) -> list[tuple[str, str]]:
    """Sync a single harness-owned directory from `src_root` to `dst_root`."""
    changes: list[tuple[str, str]] = []
    src_dir = src_root / rel_dir
    dst_dir = dst_root / rel_dir
    if not src_dir.is_dir():
        return changes

    src_files: set[Path] = set()
    for path in sorted(src_dir.rglob("*")):
        if path.is_file():
            rel = path.relative_to(src_root)
            if is_ignored_sync_path(rel):
                continue
            src_files.add(rel)
            action = copy_file_if_changed(path, dst_root / rel, dry_run=dry_run)
            if action:
                changes.append((action, rel.as_posix()))

    if dst_dir.is_dir():
        for path in sorted(dst_dir.rglob("*"), reverse=True):
            rel = path.relative_to(dst_root)
            if is_ignored_sync_path(rel):
                continue
            rel_posix = rel.as_posix()
            if path.is_file() and rel not in src_files:
                if preserve_unrecorded and (
                    recorded_files is None or rel_posix not in recorded_files
                ):
                    continue
                changes.append(("delete", rel_posix))
                if not dry_run:
                    path.unlink()
            elif path.is_dir() and not dry_run and not any(path.iterdir()):
                path.rmdir()

    return changes


def sync_harness_files(
    src_root: Path,
    dst_root: Path,
    dry_run: bool = False,
    mode: str = "normal",
    recorded_files: dict[str, str] | None = None,
) -> list[tuple[str, str]]:
    """Sync harness-owned directories and files from `src_root` to `dst_root`."""
    changes: list[tuple[str, str]] = []

    for rel_dir in HARNESS_OWNED_DIRS:
        # In release mode, bin/ and .mise/ can hold project-owned tracked files.
        preserve_unrecorded = mode == "release" and rel_dir in ("bin", ".mise")
        changes.extend(
            sync_dir_tree(
                src_root,
                dst_root,
                rel_dir,
                dry_run=dry_run,
                preserve_unrecorded=preserve_unrecorded,
                recorded_files=recorded_files,
            )
        )

    for rel_file in HARNESS_OWNED_FILES:
        if mode == "release" and rel_file == ".gitignore":
            action = ensure_release_gitignore(
                dst_root, src_root=src_root, dry_run=dry_run
            )
            if action:
                changes.append((action, ".gitignore"))
            continue
        src_file = src_root / rel_file
        if not src_file.is_file():
            continue
        action = copy_file_if_changed(src_file, dst_root / rel_file, dry_run=dry_run)
        if action:
            changes.append((action, rel_file))

    return changes


def sync_release_hydrate_files(src_root: Path, dst_root: Path) -> None:
    """Populate missing gitignored upstream harness files on an existing release clone."""
    for rel_dir in RELEASE_HYDRATE_DIRS:
        sync_dir_tree(src_root, dst_root, rel_dir, dry_run=False)

    for rel_file in RELEASE_HYDRATE_FILES:
        src_file = src_root / rel_file
        if src_file.is_file():
            copy_file_if_changed(src_file, dst_root / rel_file, dry_run=False)

    # Populate shared toolchain/seed files only when missing so tracked configs are preserved.
    for rel_dir in (".mise",):
        src_dir = src_root / rel_dir
        if src_dir.is_dir():
            for path in sorted(src_dir.rglob("*")):
                if path.is_file():
                    rel = path.relative_to(src_root)
                    if is_ignored_sync_path(rel):
                        continue
                    dst_path = dst_root / rel
                    if not dst_path.exists():
                        copy_file_if_changed(path, dst_path, dry_run=False)

    for rel_file in (*HARNESS_OWNED_FILES, *SHARED_BIN_FILES, *PROJECT_SEED_FILES):
        if rel_file in RELEASE_HYDRATE_FILES or rel_file == ".gitignore":
            continue
        src_file = src_root / rel_file
        dst_file = dst_root / rel_file
        if src_file.is_file() and not dst_file.exists():
            copy_file_if_changed(src_file, dst_file, dry_run=False)


def default_init_source() -> str:
    """Return local harness root if running inside a harness repo, else DEFAULT_UPSTREAM."""
    repo_root = Path(__file__).resolve().parent.parent
    if (repo_root / "AGENTS.template.md").is_file() and (
        repo_root / "README.template.md"
    ).is_file():
        return str(repo_root)
    return DEFAULT_UPSTREAM


def prepare_downstream_project_files(
    target_root: Path,
    name: str,
    description: str,
    in_place: bool,
    preserve_existing: bool = False,
    mode: str = "normal",
) -> None:
    """Write clean project-owned starter files and remove harness-only leftovers."""
    if in_place:
        host_dir = target_root / "host"
        if host_dir.exists():
            shutil.rmtree(host_dir)
        tests_dir = target_root / "tests"
        if (tests_dir / "run.py").is_file():
            shutil.rmtree(tests_dir)
        for rel in ("CHANGELOG.md", "agent-context/releasing.md", "docs/releasing.md"):
            stale_file = target_root / rel
            if stale_file.is_file():
                stale_file.unlink()

    templates_dir = target_root / "templates"

    if not (preserve_existing and mode == "release"):
        for template_name, target_name in (
            ("release-please-config.json", "release-please-config.json"),
            ("release-please-manifest.json", ".release-please-manifest.json"),
        ):
            template_file = templates_dir / template_name
            dst_file = target_root / target_name
            if not template_file.is_file():
                raise FileNotFoundError(f"Missing starter template: {template_file}")
            if (in_place and not preserve_existing) or not dst_file.exists():
                shutil.copy2(template_file, dst_file)

    decisions_dir = target_root / "decisions"
    decisions_dir.mkdir(parents=True, exist_ok=True)
    if in_place:
        for item in decisions_dir.glob("*.md"):
            if item.name != "README.md":
                item.unlink()
    decisions_readme = decisions_dir / "README.md"
    if not preserve_existing or not decisions_readme.exists():
        decisions_readme.write_text(
            render_starter_file(
                templates_dir,
                "decisions-README.md",
                name,
                description,
                mode=mode,
            ),
            encoding="utf-8",
        )

    agent_context_dir = target_root / "agent-context"
    agent_context_dir.mkdir(parents=True, exist_ok=True)
    memory_dir = agent_context_dir / "memory"
    memory_dir.mkdir(parents=True, exist_ok=True)
    memory_file = memory_dir / "MEMORY.md"
    if (in_place and not preserve_existing) or not memory_file.exists():
        memory_file.write_text("", encoding="utf-8")

    agents_project = agent_context_dir / "AGENTS.project.md"
    if not preserve_existing or not agents_project.exists():
        agents_project.write_text(
            render_starter_file(
                templates_dir, "AGENTS.project.md", name, description, mode=mode
            ),
            encoding="utf-8",
        )

    readme_project = agent_context_dir / "README.project.md"
    if not preserve_existing or not readme_project.exists():
        readme_project.write_text(
            render_starter_file(
                templates_dir, "README.project.md", name, description, mode=mode
            ),
            encoding="utf-8",
        )


def find_sync_conflicts(
    dst_root: Path,
    changes: list[tuple[str, str]],
    recorded_files: dict[str, str],
    mode: str = "normal",
) -> list[tuple[str, str]]:
    """Return (rel_path, reason) for planned changes that would clobber non-harness edits."""
    conflicts: list[tuple[str, str]] = []
    for action, rel in changes:
        # In release mode only the managed block inside .gitignore is touched.
        if mode == "release" and rel == ".gitignore":
            continue
        dst_path = dst_root / rel
        recorded_hash = recorded_files.get(rel)
        if action == "update":
            if recorded_hash is None:
                conflicts.append((rel, "not tracked in .harness.json; would overwrite"))
            elif dst_path.is_file() and sha256_file(dst_path) != recorded_hash:
                conflicts.append((rel, "locally modified; would overwrite"))
        elif action == "delete":
            if recorded_hash is None:
                conflicts.append((rel, "not from harness; would delete"))
            elif dst_path.is_file() and sha256_file(dst_path) != recorded_hash:
                conflicts.append((rel, "locally modified; would delete"))
    return conflicts


def has_uncommitted_changes(repo_dir: Path) -> bool:
    """Return True if `repo_dir` is a git repo with uncommitted changes."""
    if not (repo_dir / ".git").exists() or not shutil.which("git"):
        return False
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "status", "--porcelain"],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode == 0 and bool(result.stdout.strip())


def untrack_release_excluded_paths(repo_dir: Path) -> None:
    """Remove any tracked release-excluded paths from the git index without deleting files."""
    if not (repo_dir / ".git").exists() or not shutil.which("git"):
        return
    tracked = subprocess.run(
        ["git", "-C", str(repo_dir), "ls-files", "--", *RELEASE_EXCLUDED_PATHS],
        capture_output=True,
        text=True,
        check=False,
    )
    if tracked.returncode != 0 or not tracked.stdout.strip():
        return
    subprocess.run(
        [
            "git",
            "-C",
            str(repo_dir),
            "rm",
            "-r",
            "--cached",
            "--ignore-unmatch",
            "--quiet",
            "--",
            *RELEASE_EXCLUDED_PATHS,
        ],
        check=False,
        capture_output=True,
    )


def resolve_git_filter_repo_cmd(target_root: Path) -> list[str]:
    """Locate a runnable `git-filter-repo` command across target, harness, or PATH."""
    mise_bin = shutil.which("mise")
    if mise_bin:
        harness_root = Path(__file__).resolve().parent.parent
        for cfg_dir in (target_root, harness_root):
            res = subprocess.run(
                [mise_bin, "-C", str(cfg_dir), "which", "git-filter-repo"],
                capture_output=True,
                text=True,
                check=False,
            )
            if res.returncode == 0:
                candidate = res.stdout.strip()
                if candidate and Path(candidate).is_file():
                    return [candidate]

    direct = shutil.which("git-filter-repo")
    if direct:
        probe = subprocess.run(
            [direct, "--version"],
            cwd=target_root,
            capture_output=True,
            text=True,
            check=False,
        )
        if probe.returncode == 0:
            return [direct]

    raise RuntimeError(
        "git-filter-repo is required for 'release'. Run 'mise install' first."
    )


def backup_release_excluded_files(
    root: Path, backup_dir: Path
) -> list[tuple[str, bool]]:
    """Back up existing release-excluded files and directories under `root`."""
    backed_up: list[tuple[str, bool]] = []
    for rel in RELEASE_EXCLUDED_PATHS:
        clean_rel = rel.rstrip("/")
        src = root / clean_rel
        if not src.exists():
            continue
        dst = backup_dir / clean_rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if src.is_dir():
            shutil.copytree(
                src,
                dst,
                symlinks=True,
                ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo"),
            )
            backed_up.append((clean_rel, True))
        elif src.is_file():
            shutil.copy2(src, dst)
            backed_up.append((clean_rel, False))
    return backed_up


def restore_release_excluded_files(
    root: Path, backup_dir: Path, backed_up: list[tuple[str, bool]]
) -> None:
    """Restore backed-up release-excluded files and directories into `root`."""
    for rel, is_dir in backed_up:
        src = backup_dir / rel
        dst = root / rel
        if is_dir:
            shutil.copytree(src, dst, symlinks=True, dirs_exist_ok=True)
        else:
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)


def build_filter_repo_callback() -> str:
    """Return the `--file-info-callback` Python body stripping normal-only blocks."""
    pattern_literal = repr(NORMAL_ONLY_RE.pattern)
    return (
        "if filename in (b'README.md', b'decisions/README.md') and mode != b'120000':\n"
        "    data = value.get_contents_by_identifier(blob_id)\n"
        "    if not value.is_binary(data):\n"
        "        import re\n"
        "        pattern = value.data.get('harness_normal_only_re')\n"
        "        if pattern is None:\n"
        f"            pattern = re.compile({pattern_literal}, re.DOTALL)\n"
        "            value.data['harness_normal_only_re'] = pattern\n"
        "        text = data.decode('utf-8', errors='replace')\n"
        "        if pattern.search(text):\n"
        "            cleaned = pattern.sub('', text)\n"
        "            cleaned = re.sub(r'\\n{3,}', '\\n\\n', cleaned).strip('\\r\\n')\n"
        "            cleaned = f'{cleaned}\\n' if cleaned else ''\n"
        "            if cleaned != text:\n"
        "                blob_id = value.insert_file_with_contents(cleaned.encode('utf-8'))\n"
        "return (filename, mode, blob_id)\n"
    )


def commit_release_changes(repo_dir: Path) -> None:
    """Stage release-mode tracked updates (.gitignore, README.md, decisions/README.md) and commit."""
    to_add = [".gitignore"]
    for rel in ("README.md", "decisions/README.md"):
        if (repo_dir / rel).is_file():
            to_add.append(rel)
    subprocess.run(
        ["git", "-C", str(repo_dir), "add", "--", *to_add],
        check=True,
        capture_output=True,
        text=True,
    )
    diff_cached = subprocess.run(
        ["git", "-C", str(repo_dir), "diff", "--cached", "--quiet"],
        capture_output=True,
        check=False,
    )
    if diff_cached.returncode == 0:
        return

    cmd = ["git", "-C", str(repo_dir)]
    name_cfg = subprocess.run(
        ["git", "-C", str(repo_dir), "config", "user.name"],
        capture_output=True,
        text=True,
        check=False,
    )
    email_cfg = subprocess.run(
        ["git", "-C", str(repo_dir), "config", "user.email"],
        capture_output=True,
        text=True,
        check=False,
    )
    if not name_cfg.stdout.strip() or not email_cfg.stdout.strip():
        author_res = subprocess.run(
            ["git", "-C", str(repo_dir), "log", "-1", "--format=%an%n%ae"],
            capture_output=True,
            text=True,
            check=False,
        )
        lines = author_res.stdout.splitlines()
        fallback_name = lines[0] if len(lines) > 0 and lines[0] else "harness"
        fallback_email = (
            lines[1] if len(lines) > 1 and lines[1] else "harness@example.invalid"
        )
        cmd.extend(
            ["-c", f"user.name={fallback_name}", "-c", f"user.email={fallback_email}"]
        )

    cmd.extend(["commit", "-m", "chore: configure harness release mode"])
    res = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if res.returncode != 0:
        raise RuntimeError(
            f"Failed to commit release mode changes: {res.stderr.strip() or res.stdout.strip()}"
        )


def cmd_render(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    ok = render_root(root, check=args.check)
    return 0 if ok else 1


def cmd_init(args: argparse.Namespace) -> int:
    if args.in_place:
        target_root = Path(args.target or ".").resolve()
        if not (target_root / "AGENTS.template.md").is_file():
            print(
                f"Error: {target_root} does not look like a harness clone (missing AGENTS.template.md).",
                file=sys.stderr,
            )
            return 1
        mode = resolve_effective_mode(target_root, requested_release=args.release)
        project_name = args.name or target_root.name
        description = args.description or "Project description."
        commit = get_git_commit(target_root)
        version = (
            read_harness_version(target_root)
            if (target_root / "bin" / "harness.py").is_file()
            else HARNESS_VERSION
        )
        if args.ref:
            ref = args.ref
        elif args.source:
            ref = resolve_default_ref(args.source)
        else:
            ref = resolve_default_ref(str(target_root))
        prepare_downstream_project_files(
            target_root, project_name, description, in_place=True, mode=mode
        )
        if mode == "release":
            ensure_release_gitignore(target_root)
            untrack_release_excluded_paths(target_root)
        write_harness_meta(
            target_root,
            upstream=args.source or DEFAULT_UPSTREAM,
            ref=ref,
            commit=commit,
            mode=mode,
            version=version,
        )
        render_root(target_root, check=False, mode=mode)
        print(
            f"Initialized downstream project in-place ({mode} mode, v{version}) at {target_root}"
        )
        return 0

    if not args.target:
        print(
            "Error: target directory is required unless --in-place is set.",
            file=sys.stderr,
        )
        return 1

    target_root = Path(args.target).resolve()
    target_root.mkdir(parents=True, exist_ok=True)
    mode = resolve_effective_mode(target_root, requested_release=args.release)
    has_git_dir = (target_root / ".git").exists()
    existing_clone = has_git_dir and (
        get_git_commit(target_root) is not None
        or (target_root / "agent-context" / "AGENTS.project.md").is_file()
        or (target_root / "decisions").is_dir()
    )

    project_name = args.name or target_root.name
    description = args.description or "Project description."
    source = args.source or default_init_source()
    ref = args.ref or resolve_default_ref(source)

    with tempfile.TemporaryDirectory(prefix="harness-init-") as tmp:
        src_root, upstream_url, commit = resolve_upstream(source, ref, Path(tmp))
        target_version = read_harness_version(src_root)
        if existing_clone and mode == "release":
            sync_release_hydrate_files(src_root, target_root)
        else:
            sync_harness_files(src_root, target_root, dry_run=False, mode=mode)
            for seed in PROJECT_SEED_FILES:
                src_seed = src_root / seed
                dst_seed = target_root / seed
                if src_seed.is_file() and not dst_seed.exists():
                    shutil.copy2(src_seed, dst_seed)

        if mode == "release":
            ensure_release_gitignore(target_root, src_root=src_root)
        collected_hashes = (
            collect_harness_hashes(src_root)
            if existing_clone and mode == "release"
            else collect_harness_hashes(target_root, src_root=src_root)
        )

    prepare_downstream_project_files(
        target_root,
        project_name,
        description,
        in_place=False,
        preserve_existing=existing_clone,
        mode=mode,
    )
    write_harness_meta(
        target_root,
        upstream=upstream_url,
        ref=ref,
        commit=commit,
        mode=mode,
        files=collected_hashes,
        version=target_version,
    )
    only_targets = (
        {"AGENTS.md"}
        if existing_clone and (target_root / "README.md").is_file()
        else None
    )
    render_root(target_root, check=False, mode=mode, only_targets=only_targets)

    if not has_git_dir and shutil.which("git"):
        subprocess.run(
            ["git", "init", "-b", "main", str(target_root)],
            check=False,
            capture_output=True,
        )

    print(
        f"Initialized downstream project ({mode} mode, v{target_version}) at {target_root}"
    )
    return 0


def cmd_update(args: argparse.Namespace) -> int:
    target_root = Path(args.root).resolve()
    if (target_root / "host").exists():
        print(
            "Error: refusing to run 'update' inside the upstream harness repository (host/ exists).",
            file=sys.stderr,
        )
        return 1

    if not args.dry_run and not args.force and has_uncommitted_changes(target_root):
        print(
            "Error: working tree has uncommitted changes. Commit or stash them, or pass --force.",
            file=sys.stderr,
        )
        return 1

    meta = read_harness_meta(target_root)
    current_version_str = str(meta["version"])
    current_semver = parse_semver(current_version_str)
    mode = "release" if has_release_gitignore_block(target_root) else str(meta["mode"])
    source = args.source or str(meta["upstream"])
    ref = args.ref or resolve_default_ref(source)
    recorded_files = meta["files"]
    assert isinstance(recorded_files, dict)

    with tempfile.TemporaryDirectory(prefix="harness-update-") as tmp:
        src_root, upstream_url, commit = resolve_upstream(source, ref, Path(tmp))
        target_version_str = read_harness_version(src_root)
        target_semver = parse_semver(target_version_str)
        is_major_upgrade = target_semver.major > current_semver.major
        changelog_notes = extract_changelog_notes(
            src_root / "CHANGELOG.md", current_semver, target_semver
        )
        planned = sync_harness_files(
            src_root,
            target_root,
            dry_run=True,
            mode=mode,
            recorded_files=recorded_files,
        )

        if args.dry_run:
            if not planned:
                print(
                    f"Already up to date (ref: {ref}, version: {current_version_str} -> {target_version_str})."
                )
            else:
                print(
                    f"[dry-run] target: {ref} ({current_version_str} -> {target_version_str})"
                )
                for action, rel in planned:
                    print(f"[dry-run] {action}: {rel}")
            if changelog_notes:
                print(
                    f"\nChangelog ({current_version_str} -> {target_version_str}):\n{changelog_notes}"
                )
            if is_major_upgrade and not (args.major or args.force):
                print(
                    f"[dry-run] warning: major version upgrade ({current_version_str} -> {target_version_str}) requires --major (or --force) to apply.",
                    file=sys.stderr,
                )
            conflicts = find_sync_conflicts(
                target_root, planned, recorded_files, mode=mode
            )
            for rel, reason in conflicts:
                print(f"[dry-run] warning: {rel} ({reason})", file=sys.stderr)
            return 0

        if is_major_upgrade and not (args.major or args.force):
            if changelog_notes:
                print(
                    f"Changelog ({current_version_str} -> {target_version_str}):\n{changelog_notes}\n",
                    file=sys.stderr,
                )
            print(
                f"Error: major harness upgrade ({current_version_str} -> {target_version_str}) blocked because it may include breaking changes. Review the release notes above and re-run with --major (or --force) to apply.",
                file=sys.stderr,
            )
            return 1

        conflicts = find_sync_conflicts(target_root, planned, recorded_files, mode=mode)
        if conflicts and not args.force:
            print(
                "Error: update blocked because non-harness or locally modified files would be overwritten/deleted:",
                file=sys.stderr,
            )
            for rel, reason in conflicts:
                print(f"  - {rel} ({reason})", file=sys.stderr)
            print(
                "Resolve or back up these files, or re-run with --force to overwrite.",
                file=sys.stderr,
            )
            return 1

        if changelog_notes:
            print(
                f"Changelog ({current_version_str} -> {target_version_str}):\n{changelog_notes}\n"
            )

        changes = sync_harness_files(
            src_root,
            target_root,
            dry_run=False,
            mode=mode,
            recorded_files=recorded_files,
        )
        collected_hashes = collect_harness_hashes(target_root, src_root=src_root)

    for action, rel in changes:
        print(f"{action}: {rel}")

    write_harness_meta(
        target_root,
        upstream=upstream_url,
        ref=ref,
        commit=commit,
        mode=mode,
        files=collected_hashes,
        version=target_version_str,
    )
    render_root(target_root, check=False, mode=mode)

    if changes:
        print(
            f"Updated {len(changes)} harness file(s) to v{target_version_str}. Review changes with 'git diff' before committing."
        )
    else:
        print(f"Harness files are up to date (v{target_version_str}).")
    return 0


def cmd_release(args: argparse.Namespace) -> int:
    target_root = Path(args.root).resolve()
    if (target_root / "host").exists():
        print(
            "Error: refusing to run 'release' inside the upstream harness repository (host/ exists).",
            file=sys.stderr,
        )
        return 1

    if not (target_root / ".git").exists() or get_git_commit(target_root) is None:
        print(
            f"Error: {target_root} must be an existing git repository with commits.",
            file=sys.stderr,
        )
        return 1

    if has_uncommitted_changes(target_root):
        print(
            "Error: working tree has uncommitted changes. Commit or stash them before running 'release'.",
            file=sys.stderr,
        )
        return 1

    if (target_root / "AGENTS.project.md").exists() or (
        target_root / "README.project.md"
    ).exists():
        print(
            "Error: found root AGENTS.project.md or README.project.md; migrate project inputs under agent-context/ before running 'release'.",
            file=sys.stderr,
        )
        return 1

    filter_cmd = resolve_git_filter_repo_cmd(target_root)
    meta = read_harness_meta(target_root)

    origin_res = subprocess.run(
        ["git", "-C", str(target_root), "remote", "get-url", "origin"],
        capture_output=True,
        text=True,
        check=False,
    )
    origin_url = origin_res.stdout.strip() if origin_res.returncode == 0 else None

    path_args: list[str] = []
    for rel in RELEASE_EXCLUDED_PATHS:
        path_args.extend(["--path", rel])

    with tempfile.TemporaryDirectory(prefix="harness-release-") as tmp:
        tmp_path = Path(tmp)
        backup_dir = tmp_path / "backup"
        backup_dir.mkdir()
        backed_up = backup_release_excluded_files(target_root, backup_dir)

        cb_path = tmp_path / "readme_callback.py"
        cb_path.write_text(build_filter_repo_callback(), encoding="utf-8")

        cmd = [
            *filter_cmd,
            "--force",
            "--invert-paths",
            *path_args,
            "--file-info-callback",
            str(cb_path),
        ]
        res = subprocess.run(
            cmd,
            cwd=target_root,
            capture_output=True,
            text=True,
            check=False,
        )
        if res.returncode != 0:
            restore_release_excluded_files(target_root, backup_dir, backed_up)
            raise RuntimeError(
                f"git filter-repo failed: {res.stderr.strip() or res.stdout.strip()}"
            )

        if origin_url:
            has_origin = subprocess.run(
                ["git", "-C", str(target_root), "remote", "get-url", "origin"],
                capture_output=True,
                check=False,
            )
            if has_origin.returncode != 0:
                subprocess.run(
                    [
                        "git",
                        "-C",
                        str(target_root),
                        "remote",
                        "add",
                        "origin",
                        origin_url,
                    ],
                    check=True,
                    capture_output=True,
                )

        restore_release_excluded_files(target_root, backup_dir, backed_up)

    ensure_release_gitignore(target_root)
    prior_files = meta["files"] if isinstance(meta.get("files"), dict) else {}
    allowed_shared = {
        "bin/harness.py",
        "bin/sync-labels",
        "bin/wait-pr-checks",
        *SHARED_BIN_FILES,
        *(str(k) for k in prior_files if str(k).startswith(("bin/", ".mise/"))),
    }
    write_harness_meta(
        target_root,
        upstream=str(meta["upstream"]),
        ref=str(meta["ref"]),
        commit=str(meta["commit"]),
        mode="release",
        version=str(meta["version"]),
        files=collect_harness_hashes(
            target_root,
            allowed_shared_files=allowed_shared,
            filter_mise=bool(prior_files),
        ),
    )

    targets_to_render: set[str] = set()
    for template_name, project_name, output_name in RENDER_TARGETS:
        if (target_root / template_name).is_file() and (
            target_root / project_name
        ).is_file():
            targets_to_render.add(output_name)
    if targets_to_render:
        render_root(
            target_root,
            check=False,
            mode="release",
            only_targets=targets_to_render,
        )

    decisions_readme = target_root / "decisions" / "README.md"
    if decisions_readme.is_file():
        current_decisions = decisions_readme.read_text(encoding="utf-8")
        cleaned_decisions = strip_normal_only_sections(
            current_decisions, mode="release"
        )
        if cleaned_decisions != current_decisions:
            decisions_readme.write_text(cleaned_decisions, encoding="utf-8")

    commit_release_changes(target_root)
    if has_uncommitted_changes(target_root):
        raise RuntimeError("Working tree is not clean after release conversion.")

    print(f"Converted {target_root} to release mode.")
    if origin_url:
        print("History was rewritten; push with 'git push --force --all' when ready.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="harness",
        description="Initialize, update, render, and release downstream harness projects.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    render_p = subparsers.add_parser(
        "render",
        help="Render AGENTS.md and README.md from *.template.md and agent-context/*.project.md.",
    )
    render_p.add_argument(
        "--check",
        action="store_true",
        help="Verify rendered files are up to date without modifying them.",
    )
    render_p.add_argument(
        "--root",
        default=os.getcwd(),
        help="Repository root directory (default: current working directory).",
    )

    init_p = subparsers.add_parser(
        "init",
        help="Initialize a new downstream project from harness.",
    )
    init_p.add_argument(
        "target",
        nargs="?",
        help="Target directory for the new project (optional with --in-place).",
    )
    init_p.add_argument(
        "--name", help="Project title for starter agent-context/*.project.md files."
    )
    init_p.add_argument(
        "--description",
        help="Short project description for starter agent-context/*.project.md files.",
    )
    init_p.add_argument(
        "--source",
        help=f"Upstream harness path or git URL (default: local repo or {DEFAULT_UPSTREAM}).",
    )
    init_p.add_argument(
        "--ref",
        default=None,
        help=f"Upstream git tag or branch (default: latest vX.Y.Z tag, or {DEFAULT_REF} if none).",
    )
    init_p.add_argument(
        "--in-place",
        action="store_true",
        help="Convert an existing harness clone/template repo in place.",
    )
    init_p.add_argument(
        "--release",
        action="store_true",
        help="Initialize in release mode (exclude upstream harness files from git).",
    )

    update_p = subparsers.add_parser(
        "update",
        help="Pull and apply upstream harness updates without overwriting project files.",
    )
    update_p.add_argument(
        "--source",
        help="Upstream harness path or git URL (default: from .harness.json).",
    )
    update_p.add_argument(
        "--ref",
        default=None,
        help=f"Upstream git tag or branch (default: latest vX.Y.Z tag, or {DEFAULT_REF} if none).",
    )
    update_p.add_argument(
        "--dry-run",
        action="store_true",
        help="Show planned file changes without modifying anything.",
    )
    update_p.add_argument(
        "--major",
        action="store_true",
        help="Allow major version upgrades (target.major > current.major).",
    )
    update_p.add_argument(
        "--force",
        action="store_true",
        help="Bypass major-version guard, dirty-tree check, and file conflict checks.",
    )
    update_p.add_argument(
        "--root",
        default=os.getcwd(),
        help="Downstream repository root (default: current working directory).",
    )

    release_p = subparsers.add_parser(
        "release",
        help="Convert an existing normal-mode repository to release mode and purge harness history.",
    )
    release_p.add_argument(
        "--root",
        default=os.getcwd(),
        help="Downstream repository root (default: current working directory).",
    )

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "render":
            return cmd_render(args)
        if args.command == "init":
            return cmd_init(args)
        if args.command == "update":
            return cmd_update(args)
        if args.command == "release":
            return cmd_release(args)
    except (FileNotFoundError, RuntimeError, TypeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
