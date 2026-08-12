#!/usr/bin/env python3
"""Bootstrap the hand-maintained changelog into Changie version files.

Temporary release-closeout helper. It preserves the existing changelog header
and every historical release section, removes the empty legacy v0.2.0 marker
that Go's filepath.Ext misclassifies, and lets Changie own the final merge.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGES = ROOT / ".changes"
CHANGELOG = ROOT / "CHANGELOG.md"
VERSION_HEADING = re.compile(
    r"^## \[(?P<version>[0-9]+\.[0-9]+\.[0-9]+(?:-[^\]]+)?)\] - [^\n]+$",
    re.MULTILINE,
)


def main() -> None:
    content = CHANGELOG.read_text(encoding="utf-8")
    matches = list(VERSION_HEADING.finditer(content))
    if not matches:
        raise SystemExit("Changie bootstrap found no historical release sections")

    header = content[: matches[0].start()].rstrip() + "\n"
    if "## [Unreleased]" not in header:
        raise SystemExit("Changie bootstrap header does not retain [Unreleased]")
    (CHANGES / "header.md").write_text(header, encoding="utf-8")

    versions: list[str] = []
    for index, match in enumerate(matches):
        version = match.group("version")
        start = match.start()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(content)
        section = content[start:end].strip() + "\n"
        path = CHANGES / f"v{version}.md"
        path.write_text(section, encoding="utf-8")
        versions.append(version)

    legacy = CHANGES / "v0.2.0"
    if legacy.exists():
        if legacy.read_bytes():
            raise SystemExit("legacy .changes/v0.2.0 marker is not empty")
        legacy.unlink()

    if "0.2.0" in versions:
        raise SystemExit("premature 0.2.0 section survived release closeout")
    if versions[0] != "0.1.11":
        raise SystemExit(f"expected latest historical version 0.1.11, got {versions[0]}")

    print(f"changie-bootstrap: retained {len(versions)} historical releases")


if __name__ == "__main__":
    main()
