#!/usr/bin/env python3
"""Generate the retained Changie history corpus from CHANGELOG.md (#3160).

Deterministically splits the reviewed historical changelog into one
version file per release under ``.changes/`` in the exact shape
``changie merge`` consumes (``versionFormat`` and ``versionExt`` from
``.changie.yaml``), plus the retained header file. The authored bytes
are copied verbatim so the corpus stays reviewable; this script is a
verifier/generator whose output is committed, not an unreviewed
conversion authority.

Treatment decisions (documented for review; each is a typed choice,
not a silent normalization):

- ``[Unreleased]`` section: excluded from version files. Changie owns
  unreleased state through live fragments; a retained version file for
  it would double-count.
- Release dates: taken from the reviewed heading verbatim
  (``## [x.y.z] - YYYY-MM-DD``). Duplicate or missing dates are hard
  errors — the reviewed changelog is the authority and must be fixed
  there first.
- Pre-Changie formatting: preserved byte-for-byte within each section
  body (kind headings, bullets, links, reference definitions at section
  footers stay inside their release's file).
- Empty releases: a release heading with an empty body is retained as a
  version file with an empty body — an explicit marker, not elided.
- Historical entries outside the current kind vocabulary: retained
  verbatim; the corpus records history, it does not re-classify it.

Usage:
    python scripts/generate-changie-history.py            # generate
    python scripts/generate-changie-history.py --check    # verify only
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"
CHANGES_DIR = ROOT / ".changes"
HEADER_FILE = CHANGES_DIR / "header.md"

RELEASE_HEADING = re.compile(r"^## \[(?P<version>[^\]]+)\](?: - (?P<date>\d{4}-\d{2}-\d{2}))?\s*$")
VERSION_FORMAT = "## [{version}] - {date}"


class Diagnostic(Exception):
    pass


def split_changelog(text: str) -> tuple[str, list[tuple[str, str | None, str]]]:
    """Split into (header_prose, [(version, date, body)]) with verbatim bodies."""
    lines = text.splitlines(keepends=True)
    header: list[str] = []
    sections: list[tuple[str, str | None, list[str]]] = []
    current: tuple[str, str | None, list[str]] | None = None
    seen: set[str] = set()
    for line in lines:
        match = RELEASE_HEADING.match(line.rstrip("\r\n"))
        if match:
            version = match.group("version")
            date = match.group("date")
            if version in seen:
                raise Diagnostic(f"duplicate release heading [{version}] in CHANGELOG.md")
            if version != "Unreleased" and date is None:
                raise Diagnostic(f"release heading [{version}] lacks a reviewed date")
            seen.add(version)
            current = (version, date, [])
            sections.append(current)
            continue
        if current is None:
            header.append(line)
        else:
            current[2].append(line)
    if not any(version != "Unreleased" for version, _, _ in sections):
        raise Diagnostic("no versioned releases found in CHANGELOG.md")
    header_text = "".join(header)
    body_sections = [(v, d, "".join(b)) for (v, d, b) in sections if v != "Unreleased"]
    return header_text, body_sections


def normalize_newlines(text: str) -> str:
    return text.replace("\r\n", "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify corpus is current; write nothing")
    args = parser.parse_args()

    raw = CHANGELOG.read_text(encoding="utf-8")
    header, sections = split_changelog(normalize_newlines(raw))
    CHANGES_DIR.mkdir(exist_ok=True)
    expected: dict[Path, str] = {}
    # The header file carries the reviewed intro prose so `changie
    # merge` reproduces the changelog byte-for-byte when headerPath is
    # configured to this file.
    expected[HEADER_FILE] = header if header.endswith("\n") else header + "\n"
    for version, date, body in sections:
        assert date is not None  # split_changelog enforces this for releases
        heading = VERSION_FORMAT.format(version=version, date=date)
        expected[CHANGES_DIR / f"{version}.md"] = f"{heading}\n{body}"

    drift: list[str] = []
    if args.check:
        for path, content in sorted(expected.items()):
            if not path.is_file():
                drift.append(f"missing retained file {path.relative_to(ROOT)}")
            elif normalize_newlines(path.read_text(encoding="utf-8")) != content:
                drift.append(f"stale retained file {path.relative_to(ROOT)}")
        for path in sorted(CHANGES_DIR.glob("*.md")):
            if path not in expected and path.name != "README.md":
                drift.append(f"unretained history file {path.relative_to(ROOT)}")
        if drift:
            for line in drift:
                print(f"history corpus drift: {line}", file=sys.stderr)
            return 1
        print(f"history corpus current: {len(expected)} files verified")
        return 0

    for path, content in sorted(expected.items()):
        path.write_text(content, encoding="utf-8", newline="\n")
        print(f"wrote {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
