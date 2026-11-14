#!/usr/bin/env python3
"""Superset assertion for the final 0.2.0 changelog body (#3725).

`changie batch 0.2.0` rewrites `.changes/0.2.0.md` from the unreleased
fragments. The prospective final file that existed before the cut carried
authored bullets that no fragment reproduces, so a plain batch silently
drops them. `generate-changie-history.py --check` cannot catch that: it
derives the expected version files FROM CHANGELOG.md, so a lossy batch
still verifies.

This check pins the pre-cut bullets as a fixture and requires every one of
them to remain in the `[0.2.0]` section of CHANGELOG.md. It reddens only
when the preserved body is dropped or rewritten.

Usage:
    python scripts/test-changie-0-2-0-union.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"
FIXTURE = ROOT / "scripts" / "fixtures" / "changie-0.2.0-precut-bullets.txt"
SECTION = re.compile(r"^## \[0\.2\.0\](?: - (?P<date>\d{4}-\d{2}-\d{2}))?\s*$")


def normalize(text: str) -> str:
    return " ".join(text.split())


def section_body(text: str) -> str:
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if SECTION.match(line):
            start = index + 1
            break
    if start is None:
        raise SystemExit("CHANGELOG.md has no [0.2.0] section")
    body: list[str] = []
    for line in lines[start:]:
        if line.startswith("## ["):
            break
        body.append(line)
    return "\n".join(body)


def fixture_bullets() -> list[str]:
    bullets: list[str] = []
    for line in FIXTURE.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        bullets.append(normalize(line))
    return bullets


def main() -> int:
    body = normalize(section_body(CHANGELOG.read_text(encoding="utf-8")))
    bullets = fixture_bullets()
    if not bullets:
        print("union check: fixture has no bullets", file=sys.stderr)
        return 1
    missing = [bullet for bullet in bullets if bullet not in body]
    if missing:
        for bullet in missing:
            print(f"union check: dropped pre-cut bullet: {bullet[:120]}", file=sys.stderr)
        print(
            f"union check: {len(missing)} of {len(bullets)} pre-cut 0.2.0 bullets are missing",
            file=sys.stderr,
        )
        return 1
    print(f"union check: all {len(bullets)} pre-cut 0.2.0 bullets retained")
    return 0


if __name__ == "__main__":
    sys.exit(main())
