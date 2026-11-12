#!/usr/bin/env python3
"""Reject merge-group checkout refs that fall back to github.ref.

A workflow with a ``merge_group`` trigger must not check out a ref that
contains ``github.ref`` or ``github.head_ref`` without a ``merge_group``
term. On a merge-group event those values name the queue ref, not the
merge-group head, so the checkout would bind the wrong subject. Omitting
``ref`` is correct there because ``github.sha`` is the merge-group head.

The guard is intentionally a text scan: it reads the workflow files that
are present in the checkout and reports every offending ``ref:`` line.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

MERGE_GROUP_TRIGGER = re.compile(r"^  merge_group:", re.MULTILINE)
CHECKOUT_REF = re.compile(r"^\s*ref:\s*(?P<value>.+?)\s*$")
UNSAFE = ("github.ref", "github.head_ref")


def offending_refs(text: str) -> list[tuple[int, str]]:
    """Return (line number, line) for each unsafe checkout ref."""
    if not MERGE_GROUP_TRIGGER.search(text):
        return []
    found: list[tuple[int, str]] = []
    for number, line in enumerate(text.splitlines(), 1):
        match = CHECKOUT_REF.match(line)
        if match is None:
            continue
        value = match.group("value")
        if any(token in value for token in UNSAFE) and "merge_group" not in value:
            found.append((number, line.strip()))
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflows", type=Path, default=Path(".github/workflows"),
        help="workflow directory to scan",
    )
    args = parser.parse_args()
    failures = 0
    for path in sorted(args.workflows.glob("*.y*ml")):
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            print(f"{path}: unreadable: {error}", file=sys.stderr)
            failures += 1
            continue
        for number, line in offending_refs(text):
            print(f"{path}:{number}: {line}")
            failures += 1
    if failures:
        print(
            "merge-group checkout refs must use "
            "github.event.merge_group.head_sha or github.sha, not github.ref",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
