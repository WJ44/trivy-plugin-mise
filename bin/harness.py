#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""CLI for initializing, updating, and rendering downstream harness projects."""

from __future__ import annotations

import argparse
import filecmp
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

DEFAULT_UPSTREAM = "https://github.com/WJ44/harness.git"
DEFAULT_REF = "main"
HARNESS_META_FILE = ".harness.json"

# Directories completely owned by the harness (mirrored on init and update).
HARNESS_OWNED_DIRS = (
    "decisions/harness",
    "agent-context/harness",
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
    ".claude/settings.json",
    ".commitlintrc.json",
    ".editorconfig",
    ".gitattributes",
    ".github/labels.json",
    ".github/workflows/ci.yml",
    ".gitignore",
    ".lefthookrc",
    ".prettierignore",
    ".prettierrc.json",
    "lefthook.yml",
    "mise.lock",
    "mise.toml",
    "renovate.json",
    "setup-system.sh",
    "setup-user.sh",
    "setup.sh",
    "trivy.yaml",
)

# Files initialized from harness on init, then owned by the downstream project.
PROJECT_SEED_FILES = (".trivyignore.yaml",)

RENDER_TARGETS = (
    ("AGENTS.template.md", "AGENTS.project.md", "AGENTS.md"),
    ("README.template.md", "README.project.md", "README.md"),
)

SECTION_RE = re.compile(
    r"<!--\s*project:(?P<name>[a-zA-Z0-9_-]+):start\s*-->(?P<body>.*?)<!--\s*project:(?P=name):end\s*-->",
    re.DOTALL,
)
PLACEHOLDER_LINE_RE = re.compile(
    r"(?m)^[ \t]*\{\{project:(?P<name>[a-zA-Z0-9_-]+)\}\}[ \t]*\n?"
)
PLACEHOLDER_INLINE_RE = re.compile(r"\{\{project:(?P<name>[a-zA-Z0-9_-]+)\}\}")


def extract_project_sections(project_text: str) -> dict[str, str]:
    """Extract named `<!-- project:<name>:start/end -->` blocks from markdown."""
    sections: dict[str, str] = {}
    for match in SECTION_RE.finditer(project_text):
        name = match.group("name")
        body = match.group("body").strip("\r\n")
        sections[name] = body
    return sections


def render_template(template_text: str, project_text: str) -> str:
    """Substitute project blocks into a template and normalize blank lines."""
    sections = extract_project_sections(project_text)

    def replace_line(match: re.Match[str]) -> str:
        name = match.group("name")
        body = sections.get(name, "").strip("\r\n")
        if not body:
            return ""
        return f"{body}\n"

    rendered = PLACEHOLDER_LINE_RE.sub(replace_line, template_text)
    rendered = PLACEHOLDER_INLINE_RE.sub(
        lambda m: sections.get(m.group("name"), "").strip("\r\n"),
        rendered,
    )
    rendered = re.sub(r"\n{3,}", "\n\n", rendered)
    return rendered.rstrip("\n") + "\n"


def render_root(root: Path, check: bool = False) -> bool:
    """Render AGENTS.md and README.md under `root`. Returns True if all up-to-date/ok."""
    all_ok = True
    for template_name, project_name, output_name in RENDER_TARGETS:
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
        )
        current = output_path.read_text(encoding="utf-8") if output_path.is_file() else None

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
    templates_dir: Path, filename: str, name: str, description: str
) -> str:
    """Load a starter template file and substitute project name/description."""
    template_path = templates_dir / filename
    if not template_path.is_file():
        raise FileNotFoundError(f"Missing starter template: {template_path}")
    content = template_path.read_text(encoding="utf-8")
    return content.replace("{{project:name}}", name).replace(
        "{{project:description}}", description
    )


def is_local_source(source: str) -> bool:
    """Return True if `source` points to an existing local directory."""
    return Path(source).expanduser().is_dir()


def get_git_commit(repo_dir: Path) -> str | None:
    """Return HEAD commit SHA if `repo_dir` is a git repository, else None."""
    if not (repo_dir / ".git").exists():
        return None
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return result.stdout.strip()
    return None


def resolve_upstream(
    source: str, ref: str, tmpdir: Path
) -> tuple[Path, str, str | None]:
    """Resolve upstream harness directory and return (path, upstream_url, commit)."""
    if is_local_source(source):
        src_path = Path(source).expanduser().resolve()
        commit = get_git_commit(src_path)
        return src_path, DEFAULT_UPSTREAM, commit

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


def collect_harness_hashes(root: Path) -> dict[str, str]:
    """Compute SHA-256 hashes for all harness-owned files present under `root`."""
    hashes: dict[str, str] = {}
    for rel_dir in HARNESS_OWNED_DIRS:
        dir_path = root / rel_dir
        if not dir_path.is_dir():
            continue
        for path in sorted(dir_path.rglob("*")):
            if path.is_file():
                rel = path.relative_to(root).as_posix()
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
    files: dict[str, str] | None = None,
) -> None:
    """Write .harness.json metadata in the downstream repository."""
    meta = {
        "upstream": upstream,
        "ref": ref,
        "commit": commit or "",
        "files": files if files is not None else collect_harness_hashes(target_root),
    }
    meta_path = target_root / HARNESS_META_FILE
    meta_path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")


def read_harness_meta(target_root: Path) -> dict[str, object]:
    """Read .harness.json if present, returning defaults otherwise."""
    meta_path = target_root / HARNESS_META_FILE
    if not meta_path.is_file():
        return {
            "upstream": DEFAULT_UPSTREAM,
            "ref": DEFAULT_REF,
            "commit": "",
            "files": {},
        }
    data = json.loads(meta_path.read_text(encoding="utf-8"))
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
        "files": files,
    }


def copy_file_if_changed(
    src: Path, dst: Path, dry_run: bool = False
) -> str | None:
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


def sync_harness_files(
    src_root: Path, dst_root: Path, dry_run: bool = False
) -> list[tuple[str, str]]:
    """Sync harness-owned directories and files from `src_root` to `dst_root`."""
    changes: list[tuple[str, str]] = []

    for rel_dir in HARNESS_OWNED_DIRS:
        src_dir = src_root / rel_dir
        dst_dir = dst_root / rel_dir
        if not src_dir.is_dir():
            continue

        src_files: set[Path] = set()
        for path in sorted(src_dir.rglob("*")):
            if path.is_file():
                rel = path.relative_to(src_root)
                src_files.add(rel)
                action = copy_file_if_changed(path, dst_root / rel, dry_run=dry_run)
                if action:
                    changes.append((action, rel.as_posix()))

        if dst_dir.is_dir():
            for path in sorted(dst_dir.rglob("*"), reverse=True):
                rel = path.relative_to(dst_root)
                if path.is_file() and rel not in src_files:
                    changes.append(("delete", rel.as_posix()))
                    if not dry_run:
                        path.unlink()
                elif path.is_dir() and not dry_run and not any(path.iterdir()):
                    path.rmdir()

    for rel_file in HARNESS_OWNED_FILES:
        src_file = src_root / rel_file
        if not src_file.is_file():
            continue
        action = copy_file_if_changed(src_file, dst_root / rel_file, dry_run=dry_run)
        if action:
            changes.append((action, rel_file))

    return changes


def default_init_source() -> str:
    """Return local harness root if running inside a harness repo, else DEFAULT_UPSTREAM."""
    repo_root = Path(__file__).resolve().parent.parent
    if (repo_root / "AGENTS.template.md").is_file() and (
        repo_root / "README.template.md"
    ).is_file():
        return str(repo_root)
    return DEFAULT_UPSTREAM


def prepare_downstream_project_files(
    target_root: Path, name: str, description: str, in_place: bool
) -> None:
    """Write clean project-owned starter files and remove harness-only leftovers."""
    host_dir = target_root / "host"
    if host_dir.exists():
        shutil.rmtree(host_dir)

    templates_dir = target_root / "templates"

    decisions_dir = target_root / "decisions"
    decisions_dir.mkdir(parents=True, exist_ok=True)
    if in_place:
        for item in decisions_dir.glob("*.md"):
            if item.name != "README.md":
                item.unlink()
    (decisions_dir / "README.md").write_text(
        render_starter_file(templates_dir, "decisions-README.md", name, description),
        encoding="utf-8",
    )

    (target_root / "agent-context").mkdir(parents=True, exist_ok=True)
    memory_dir = target_root / "memory"
    memory_dir.mkdir(parents=True, exist_ok=True)
    memory_file = memory_dir / "MEMORY.md"
    if in_place or not memory_file.exists():
        memory_file.write_text("", encoding="utf-8")

    (target_root / "AGENTS.project.md").write_text(
        render_starter_file(templates_dir, "AGENTS.project.md", name, description),
        encoding="utf-8",
    )
    (target_root / "README.project.md").write_text(
        render_starter_file(templates_dir, "README.project.md", name, description),
        encoding="utf-8",
    )


def find_sync_conflicts(
    dst_root: Path,
    changes: list[tuple[str, str]],
    recorded_files: dict[str, str],
) -> list[tuple[str, str]]:
    """Return (rel_path, reason) for planned changes that would clobber non-harness edits."""
    conflicts: list[tuple[str, str]] = []
    for action, rel in changes:
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
        project_name = args.name or target_root.name
        description = args.description or "Project description."
        commit = get_git_commit(target_root)
        prepare_downstream_project_files(
            target_root, project_name, description, in_place=True
        )
        write_harness_meta(
            target_root,
            upstream=args.source or DEFAULT_UPSTREAM,
            ref=args.ref,
            commit=commit,
        )
        render_root(target_root, check=False)
        print(f"Initialized downstream project in-place at {target_root}")
        return 0

    if not args.target:
        print(
            "Error: target directory is required unless --in-place is set.",
            file=sys.stderr,
        )
        return 1

    target_root = Path(args.target).resolve()
    target_root.mkdir(parents=True, exist_ok=True)
    project_name = args.name or target_root.name
    description = args.description or "Project description."
    source = args.source or default_init_source()

    with tempfile.TemporaryDirectory(prefix="harness-init-") as tmp:
        src_root, upstream_url, commit = resolve_upstream(
            source, args.ref, Path(tmp)
        )
        sync_harness_files(src_root, target_root, dry_run=False)
        for seed in PROJECT_SEED_FILES:
            src_seed = src_root / seed
            dst_seed = target_root / seed
            if src_seed.is_file() and not dst_seed.exists():
                shutil.copy2(src_seed, dst_seed)

    prepare_downstream_project_files(
        target_root, project_name, description, in_place=False
    )
    write_harness_meta(target_root, upstream=upstream_url, ref=args.ref, commit=commit)
    render_root(target_root, check=False)

    if not (target_root / ".git").exists() and shutil.which("git"):
        subprocess.run(
            ["git", "init", "-b", "main", str(target_root)],
            check=False,
            capture_output=True,
        )

    print(f"Initialized downstream project at {target_root}")
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
    source = args.source or str(meta["upstream"])
    ref = args.ref or str(meta["ref"])
    recorded_files = meta["files"]
    assert isinstance(recorded_files, dict)

    with tempfile.TemporaryDirectory(prefix="harness-update-") as tmp:
        src_root, upstream_url, commit = resolve_upstream(source, ref, Path(tmp))
        planned = sync_harness_files(src_root, target_root, dry_run=True)

        if args.dry_run:
            if not planned:
                print("Already up to date.")
            else:
                for action, rel in planned:
                    print(f"[dry-run] {action}: {rel}")
            conflicts = find_sync_conflicts(target_root, planned, recorded_files)
            for rel, reason in conflicts:
                print(f"[dry-run] warning: {rel} ({reason})", file=sys.stderr)
            return 0

        conflicts = find_sync_conflicts(target_root, planned, recorded_files)
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

        changes = sync_harness_files(src_root, target_root, dry_run=False)

    for action, rel in changes:
        print(f"{action}: {rel}")

    write_harness_meta(target_root, upstream=upstream_url, ref=ref, commit=commit)
    render_root(target_root, check=False)

    if changes:
        print(
            f"Updated {len(changes)} harness file(s). Review changes with 'git diff' before committing."
        )
    else:
        print("Harness files are up to date.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="harness",
        description="Initialize, update, and render downstream harness projects.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    render_p = subparsers.add_parser(
        "render",
        help="Render AGENTS.md and README.md from *.template.md and *.project.md.",
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
    init_p.add_argument("--name", help="Project title for starter *.project.md files.")
    init_p.add_argument(
        "--description", help="Short project description for starter *.project.md files."
    )
    init_p.add_argument(
        "--source",
        help=f"Upstream harness path or git URL (default: local repo or {DEFAULT_UPSTREAM}).",
    )
    init_p.add_argument(
        "--ref",
        default=DEFAULT_REF,
        help=f"Upstream git branch or tag (default: {DEFAULT_REF}).",
    )
    init_p.add_argument(
        "--in-place",
        action="store_true",
        help="Convert an existing harness clone/template repo in place.",
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
        help="Upstream git branch or tag (default: from .harness.json).",
    )
    update_p.add_argument(
        "--dry-run",
        action="store_true",
        help="Show planned file changes without modifying anything.",
    )
    update_p.add_argument(
        "--force",
        action="store_true",
        help="Overwrite/delete locally modified or non-harness files in harness-owned paths.",
    )
    update_p.add_argument(
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
    except (FileNotFoundError, RuntimeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
