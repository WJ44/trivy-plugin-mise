#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Validate decision record headers, required sections, and README index tables."""

from __future__ import annotations

import argparse
import datetime as dt
import re
import sys
from dataclasses import dataclass
from pathlib import Path

RECORD_FILENAME_RE = re.compile(
    r"^(?P<num>\d{3})-(?P<slug>[a-z0-9]+(?:-[a-z0-9]+)*)\.md$"
)
H1_RE = re.compile(r"^#\s+(?P<num>\d{3}):\s+(?P<title>\S.*?)\s*$")
ANY_H1_RE = re.compile(r"^#\s+\S")
H2_RE = re.compile(r"^##\s+(?P<title>.+?)\s*$")
FENCE_RE = re.compile(r"^\s{0,3}(?P<fence>`{3,}|~{3,})")

META_START_RE = re.compile(r"^- \*\*(?P<key>[A-Za-z]+):\*\*(?:\s+(?P<val>.*))?$")
REQUIRED_META_KEYS = ("Status", "Finality", "Scope", "Date")

SUPERSEDED_RE = re.compile(
    r"^Superseded by \[(?P<num>\d{3})\]\((?P<file>\d{3}-[a-z0-9]+(?:-[a-z0-9]+)*\.md)\)$"
)
VALID_FINALITY_LEVELS = ("Provisional", "Standard", "Locked")
FINALITY_LEVEL_RE = re.compile(r"\b(?:Provisional|Standard|Locked)\b")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

REQUIRED_SECTIONS = ("Context", "Decision", "Options Considered", "Consequences")
INDEX_COLUMNS = ["ID", "Title", "Status", "Finality", "Date"]
TABLE_SEP_RE = re.compile(r"^\|?(?:\s*:?-+:?\s*\|)+(?:\s*:?-+:?\s*)?\|?$")
INDEX_ID_RE = re.compile(
    r"^\[(?P<num>\d{3})\]\((?P<file>\d{3}-[a-z0-9]+(?:-[a-z0-9]+)*\.md)\)$"
)


@dataclass(frozen=True)
class ParsedRecord:
    num: str
    filename: str
    title: str
    status: str
    expected_index_finality: str
    date: str


def display_path(path: Path, cwd: Path) -> str:
    """Return a clean relative path string when inside cwd, else the raw path."""
    try:
        return path.resolve().relative_to(cwd.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def update_fence_state(
    line: str, fence_char: str | None, fence_len: int
) -> tuple[bool, str | None, int]:
    """Track fenced code blocks; return (is_fence_line, fence_char, fence_len)."""
    fence_match = FENCE_RE.match(line)
    if not fence_match:
        return False, fence_char, fence_len

    marker = fence_match.group("fence")
    if fence_char is None:
        return True, marker[0], len(marker)

    if (
        marker[0] == fence_char
        and len(marker) >= fence_len
        and not line[fence_match.end() :].strip()
    ):
        return True, None, 0

    return True, fence_char, fence_len


def parse_finality(value: str) -> str | None:
    """Validate Finality and return the primary single-token README index value."""
    stripped = value.strip()
    if stripped in VALID_FINALITY_LEVELS:
        return stripped

    # Allow explicit slash-separated multi-level values, e.g. "Standard / Provisional".
    slash_parts = [part.strip() for part in stripped.split("/")]
    if len(slash_parts) > 1 and all(
        part in VALID_FINALITY_LEVELS for part in slash_parts
    ):
        if len(set(slash_parts)) == len(slash_parts):
            return slash_parts[0]
        return None

    # Allow "<Primary> (... <OtherLevel> ...)" as documented in decisions.md.
    paren_match = re.match(
        r"^(?P<lead>Provisional|Standard|Locked)\s*\((?P<detail>[^)]+)\)$", stripped
    )
    if not paren_match:
        return None

    lead = paren_match.group("lead")
    detail_levels = FINALITY_LEVEL_RE.findall(paren_match.group("detail"))
    ordered_levels = [lead]
    for level in detail_levels:
        if level not in ordered_levels:
            ordered_levels.append(level)
    if len(ordered_levels) < 2:
        return None
    return lead


def validate_status(
    value: str,
    record_num: str,
    existing_filenames: set[str],
    rel: str,
    errors: list[str],
) -> None:
    """Validate Status value and Superseded-by target existence."""
    if value in ("Active", "Deprecated"):
        return

    match = SUPERSEDED_RE.match(value)
    if not match:
        errors.append(
            f"{rel}: invalid Status {value!r}; expected 'Active', 'Deprecated', "
            "or 'Superseded by [MMM](MMM-slug.md)'"
        )
        return

    target_num = match.group("num")
    target_file = match.group("file")
    if not target_file.startswith(f"{target_num}-"):
        errors.append(
            f"{rel}: Status link label [{target_num}] does not match target '{target_file}'"
        )
    if target_num == record_num:
        errors.append(f"{rel}: Status cannot supersede itself ({record_num})")
    if target_file not in existing_filenames:
        errors.append(
            f"{rel}: Status target '{target_file}' does not exist in {Path(rel).parent}"
        )


def parse_h1(
    lines: list[str], filename_num: str, rel: str, errors: list[str]
) -> tuple[int, str] | None:
    """Validate and extract the leading `# NNN: <Title>` line index and title."""
    for idx, line in enumerate(lines):
        if not line.strip():
            continue
        h1_match = H1_RE.match(line)
        if not h1_match:
            errors.append(
                f"{rel}:{idx + 1}: first heading must match '# {filename_num}: <Title>'"
            )
            return None
        h1_num = h1_match.group("num")
        if h1_num != filename_num:
            errors.append(
                f"{rel}:{idx + 1}: H1 ID '{h1_num}' does not match filename prefix '{filename_num}'"
            )
        return idx, h1_match.group("title").strip()

    errors.append(f"{rel}: file is empty; expected '# {filename_num}: <Title>'")
    return None


def parse_metadata(
    lines: list[str],
    start_index: int,
    filename_num: str,
    existing_filenames: set[str],
    rel: str,
    errors: list[str],
) -> tuple[int, str, str, str]:
    """Parse and validate header metadata bullets up to the first H2 heading."""
    meta: dict[str, str] = {}
    meta_order: list[str] = []
    current_key: str | None = None
    body_start_index = len(lines)

    for idx in range(start_index, len(lines)):
        line = lines[idx]
        if H2_RE.match(line):
            body_start_index = idx
            break
        if not line.strip():
            current_key = None
            continue
        bullet_match = META_START_RE.match(line)
        if bullet_match:
            key = bullet_match.group("key")
            val = (bullet_match.group("val") or "").strip()
            if key in meta:
                errors.append(f"{rel}:{idx + 1}: duplicate metadata field '{key}'")
            else:
                meta[key] = val
                meta_order.append(key)
            current_key = key
            continue
        if current_key is not None and line.startswith(("  ", "\t")):
            meta[current_key] = f"{meta[current_key]} {line.strip()}".strip()
            continue
        errors.append(
            f"{rel}:{idx + 1}: unexpected line before '## Context'; expected metadata bullet '- **<Field>:** ...'"
        )
        current_key = None

    missing_keys = [k for k in REQUIRED_META_KEYS if k not in meta]
    extra_keys = [k for k in meta_order if k not in REQUIRED_META_KEYS]
    if missing_keys:
        errors.append(
            f"{rel}: missing required metadata field(s): {', '.join(missing_keys)}"
        )
    if extra_keys:
        errors.append(f"{rel}: unexpected metadata field(s): {', '.join(extra_keys)}")
    if not missing_keys and not extra_keys and tuple(meta_order) != REQUIRED_META_KEYS:
        errors.append(
            f"{rel}: metadata fields must be ordered {', '.join(REQUIRED_META_KEYS)} "
            f"(got {', '.join(meta_order)})"
        )

    status_val = meta.get("Status", "")
    if "Status" in meta:
        validate_status(status_val, filename_num, existing_filenames, rel, errors)

    expected_index_finality = ""
    if "Finality" in meta:
        parsed_finality = parse_finality(meta["Finality"])
        if parsed_finality is None:
            errors.append(
                f"{rel}: invalid Finality {meta['Finality']!r}; expected Provisional, "
                "Standard, Locked, or a documented multi-level combination"
            )
        else:
            expected_index_finality = parsed_finality

    if "Scope" in meta and not meta["Scope"]:
        errors.append(f"{rel}: Scope metadata field must not be empty")

    date_val = meta.get("Date", "")
    if "Date" in meta:
        if not DATE_RE.match(date_val):
            errors.append(f"{rel}: invalid Date {date_val!r}; expected YYYY-MM-DD")
        else:
            try:
                dt.date.fromisoformat(date_val)
            except ValueError:
                errors.append(
                    f"{rel}: invalid calendar Date {date_val!r}; expected YYYY-MM-DD"
                )

    return body_start_index, status_val, expected_index_finality, date_val


def validate_sections(
    lines: list[str], body_start_index: int, rel: str, errors: list[str]
) -> None:
    """Verify required H2 sections exist in order with non-empty content."""
    h2s: list[str] = []
    section_has_content: dict[str, bool] = {}
    current_h2: str | None = None
    fence_char: str | None = None
    fence_len = 0

    for idx in range(body_start_index, len(lines)):
        line = lines[idx]
        is_fence, fence_char, fence_len = update_fence_state(
            line, fence_char, fence_len
        )
        if is_fence or fence_char is not None:
            if current_h2 is not None and (is_fence or line.strip()):
                section_has_content[current_h2] = True
            continue

        if ANY_H1_RE.match(line):
            errors.append(
                f"{rel}:{idx + 1}: unexpected level-1 heading in decision body"
            )
            continue

        h2_match = H2_RE.match(line)
        if h2_match:
            title = h2_match.group("title").strip()
            h2s.append(title)
            current_h2 = title
            section_has_content.setdefault(title, False)
            continue

        if current_h2 is not None and line.strip():
            section_has_content[current_h2] = True

    missing_sections = [s for s in REQUIRED_SECTIONS if s not in h2s]
    duplicate_sections = [s for s in REQUIRED_SECTIONS if h2s.count(s) > 1]
    if missing_sections:
        errors.append(
            f"{rel}: missing required section(s): {', '.join(f'## {s}' for s in missing_sections)}"
        )
    if duplicate_sections:
        errors.append(
            f"{rel}: duplicate required section(s): {', '.join(f'## {s}' for s in duplicate_sections)}"
        )
    if not missing_sections and not duplicate_sections:
        present_required = [h for h in h2s if h in REQUIRED_SECTIONS]
        if present_required != list(REQUIRED_SECTIONS):
            errors.append(
                f"{rel}: required sections must appear in order: "
                f"{', '.join(f'## {s}' for s in REQUIRED_SECTIONS)} "
                f"(got {', '.join(f'## {s}' for s in present_required)})"
            )
    for sec in REQUIRED_SECTIONS:
        if sec in section_has_content and not section_has_content[sec]:
            errors.append(f"{rel}: section '## {sec}' must not be empty")


def parse_and_validate_record(
    path: Path,
    filename_num: str,
    existing_filenames: set[str],
    cwd: Path,
    errors: list[str],
) -> ParsedRecord | None:
    """Parse and validate one NNN-<slug>.md decision record."""
    rel = display_path(path, cwd)
    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError as exc:
        errors.append(f"{rel}: file is not valid UTF-8 ({exc})")
        return None

    lines = text.splitlines()
    h1_parsed = parse_h1(lines, filename_num, rel, errors)
    if h1_parsed is None:
        return None

    h1_index, h1_title = h1_parsed
    body_start_index, status_val, expected_index_finality, date_val = parse_metadata(
        lines, h1_index + 1, filename_num, existing_filenames, rel, errors
    )
    validate_sections(lines, body_start_index, rel, errors)

    return ParsedRecord(
        num=filename_num,
        filename=path.name,
        title=h1_title,
        status=status_val,
        expected_index_finality=expected_index_finality,
        date=date_val,
    )


def split_table_row(line: str) -> list[str]:
    """Split a Markdown pipe table row into trimmed cell strings."""
    stripped = line.strip()
    stripped = stripped.removeprefix("|")
    if stripped.endswith("|") and not stripped.endswith("\\|"):
        stripped = stripped[:-1]
    cells: list[str] = []
    buf: list[str] = []
    escaped = False
    for ch in stripped:
        if escaped:
            buf.append(ch)
            escaped = False
        elif ch == "\\":
            escaped = True
            buf.append(ch)
        elif ch == "|":
            cells.append("".join(buf).strip())
            buf.clear()
        else:
            buf.append(ch)
    cells.append("".join(buf).strip())
    return cells


def extract_index_tables(lines: list[str]) -> list[list[tuple[int, list[str]]]]:
    """Find all Markdown tables matching INDEX_COLUMNS outside code fences."""
    tables: list[list[tuple[int, list[str]]]] = []
    fence_char: str | None = None
    fence_len = 0
    idx = 0

    while idx < len(lines):
        line = lines[idx]
        is_fence, fence_char, fence_len = update_fence_state(
            line, fence_char, fence_len
        )
        if is_fence or fence_char is not None:
            idx += 1
            continue

        if "|" in line and idx + 1 < len(lines):
            header_cells = split_table_row(line)
            sep_line = lines[idx + 1].strip()
            if (
                header_cells == INDEX_COLUMNS
                and "|" in sep_line
                and TABLE_SEP_RE.match(sep_line)
                and len(split_table_row(sep_line)) == len(INDEX_COLUMNS)
            ):
                rows: list[tuple[int, list[str]]] = []
                row_idx = idx + 2
                while row_idx < len(lines):
                    row_line = lines[row_idx].strip()
                    if not row_line or "|" not in row_line:
                        break
                    rows.append((row_idx + 1, split_table_row(row_line)))
                    row_idx += 1
                tables.append(rows)
                idx = row_idx
                continue

        idx += 1

    return tables


def validate_readme_index(
    readme_path: Path,
    records_by_file: dict[str, ParsedRecord],
    existing_filenames: set[str],
    cwd: Path,
    errors: list[str],
) -> None:
    """Validate the decision index table in README.md against parsed records."""
    rel = display_path(readme_path, cwd)
    if not readme_path.is_file():
        errors.append(f"{rel}: missing decision index README.md")
        return

    try:
        lines = readme_path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as exc:
        errors.append(f"{rel}: file is not valid UTF-8 ({exc})")
        return

    index_tables = extract_index_tables(lines)
    if not index_tables:
        errors.append(
            f"{rel}: missing decision index table with header "
            f"'| {' | '.join(INDEX_COLUMNS)} |'"
        )
        return
    if len(index_tables) > 1:
        errors.append(
            f"{rel}: expected exactly one decision index table, found {len(index_tables)}"
        )
        return

    rows = index_tables[0]
    seen_files: set[str] = set()
    prev_num: int | None = None

    for lineno, cells in rows:
        if len(cells) != len(INDEX_COLUMNS):
            errors.append(
                f"{rel}:{lineno}: expected {len(INDEX_COLUMNS)} columns in index row, found {len(cells)}"
            )
            continue

        id_cell, title_cell, status_cell, finality_cell, date_cell = cells
        id_match = INDEX_ID_RE.match(id_cell)
        if not id_match:
            errors.append(
                f"{rel}:{lineno}: invalid ID cell {id_cell!r}; expected '[NNN](NNN-slug.md)'"
            )
            continue

        row_num = id_match.group("num")
        row_file = id_match.group("file")
        if not row_file.startswith(f"{row_num}-"):
            errors.append(
                f"{rel}:{lineno}: ID link label [{row_num}] does not match target '{row_file}'"
            )

        if row_file in seen_files:
            errors.append(f"{rel}:{lineno}: duplicate index row for '{row_file}'")
            continue
        seen_files.add(row_file)

        num_int = int(row_num)
        if prev_num is not None and num_int <= prev_num:
            errors.append(
                f"{rel}:{lineno}: index row {row_num} is out of numerical order "
                f"(must follow {prev_num:03d})"
            )
        prev_num = num_int

        record = records_by_file.get(row_file)
        if record is None:
            if row_file not in existing_filenames:
                errors.append(
                    f"{rel}:{lineno}: stale index row points to missing decision '{row_file}'"
                )
            continue

        if not title_cell:
            errors.append(
                f"{rel}:{lineno}: index Title cell for '{row_file}' must not be empty"
            )
        if record.status and status_cell != record.status:
            errors.append(
                f"{rel}:{lineno}: index Status {status_cell!r} for '{row_file}' "
                f"does not match record Status {record.status!r}"
            )
        if (
            record.expected_index_finality
            and finality_cell != record.expected_index_finality
        ):
            errors.append(
                f"{rel}:{lineno}: index Finality {finality_cell!r} for '{row_file}' "
                f"does not match record Finality {record.expected_index_finality!r}"
            )
        if record.date and date_cell != record.date:
            errors.append(
                f"{rel}:{lineno}: index Date {date_cell!r} for '{row_file}' "
                f"does not match record Date {record.date!r}"
            )

    for fname in sorted(existing_filenames):
        if fname not in seen_files:
            errors.append(f"{rel}: missing index row for decision record '{fname}'")


def validate_directory(directory: Path, cwd: Path) -> list[str]:
    """Validate all decision records and the README index in a directory."""
    errors: list[str] = []
    md_files = sorted(
        p for p in directory.iterdir() if p.is_file() and p.suffix == ".md"
    )

    valid_files: list[tuple[Path, str]] = []
    seen_nums: dict[str, str] = {}
    for path in md_files:
        if path.name == "README.md":
            continue
        match = RECORD_FILENAME_RE.match(path.name)
        if not match:
            errors.append(
                f"{display_path(path, cwd)}: decision record filename must match "
                "NNN-<lowercase-kebab-slug>.md"
            )
            continue
        num = match.group("num")
        if num in seen_nums:
            errors.append(
                f"{display_path(path, cwd)}: duplicate decision ID '{num}' "
                f"(already used by '{seen_nums[num]}')"
            )
        else:
            seen_nums[num] = path.name
        valid_files.append((path, num))

    existing_filenames = {path.name for path, _ in valid_files}
    records_by_file: dict[str, ParsedRecord] = {}
    for path, num in valid_files:
        record = parse_and_validate_record(path, num, existing_filenames, cwd, errors)
        if record is not None:
            records_by_file[path.name] = record

    validate_readme_index(
        directory / "README.md", records_by_file, existing_filenames, cwd, errors
    )
    return errors


def resolve_target_directories(
    paths: list[str], repo_root: Path
) -> tuple[list[Path], list[str]]:
    """Resolve CLI path arguments into unique decision directories to validate."""
    errors: list[str] = []
    dirs: list[Path] = []
    seen: set[Path] = set()

    if not paths:
        for candidate in (repo_root / "decisions", repo_root / "decisions" / "harness"):
            if candidate.is_dir():
                resolved = candidate.resolve()
                if resolved not in seen:
                    seen.add(resolved)
                    dirs.append(resolved)
        return dirs, errors

    for raw in paths:
        p = Path(raw)
        if not p.exists():
            errors.append(f"{raw}: path does not exist")
            continue
        target_dir = p.resolve() if p.is_dir() else p.resolve().parent
        if target_dir not in seen:
            seen.add(target_dir)
            dirs.append(target_dir)

    return dirs, errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Validate decision record metadata, required sections, and "
            "README.md index synchronization."
        )
    )
    parser.add_argument(
        "paths",
        nargs="*",
        help=(
            "Optional decision files or directories to validate (defaults to "
            "decisions/ and decisions/harness/ when present)."
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    repo_root = Path(__file__).resolve().parent.parent
    cwd = Path.cwd()

    dirs, errors = resolve_target_directories(args.paths, repo_root)
    for directory in dirs:
        errors.extend(validate_directory(directory, cwd))

    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
