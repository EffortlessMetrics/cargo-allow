#!/usr/bin/env python3
"""Derive affected feature-configuration matrix rows from changed files (#3905 PR D).

Input: changed file paths, one per line, on stdin (empty lines ignored);
additional paths may be passed as argv. Output (stdout): one single-line
JSON object {"mode": "rows"|"all", "rows": [configuration_id, ...]} and
exit 0 ALWAYS. Row IDs come from the checked-in projection
docs/assets/feature-configuration-matrix-v1.json (repo root = parent of
scripts/), grouped by root_package_name; this script never maintains its
own feature list.

Routing rules (the #3905 law: default WIDE; unknown impact proves ALL):

- crates/allow-rust/**            -> the allow-rust rows
- crates/allow-files/**           -> the allow-files rows
- crates/cargo-proof/**           -> the cargo-proof rows
- crates/allow-report/**          -> ALL (matrix owner; any change can
                                     alter the contract itself)
- Cargo.lock, any Cargo.toml, or any crates/** path not under the three
  product crates -> ALL (shared substrate / dependency movement has
  unknown row impact, so it fails wide)
- .github/**                      -> ALL (the lane definition itself)
- docs/**, .changes/**, *.md      -> no rows (docs-only change)
- anything else / unrecognized    -> ALL (fail wide when impact is unknown)
- empty input                     -> {"mode": "rows", "rows": []}

"all" is emitted with the concrete full row list so the consuming workflow
never branches on mode; an unreadable projection also fails wide (mode
"all") with a stderr warning. This tool is stdlib-only.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

PRODUCT_CRATE_ROWS = {
    "crates/allow-rust": "allow-rust",
    "crates/allow-files": "allow-files",
    "crates/cargo-proof": "cargo-proof",
}


def projection_path() -> Path:
    repo_root = Path(__file__).resolve().parent.parent
    return repo_root / "docs" / "assets" / "feature-configuration-matrix-v1.json"


def rows_by_package() -> dict[str, list[str]] | None:
    """Group configuration IDs by root package; None when unreadable."""
    try:
        data = json.loads(projection_path().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    grouped: dict[str, list[str]] = {}
    for row in data.get("rows", []):
        grouped.setdefault(row.get("root_package_name", ""), []).append(
            row["configuration_id"]
        )
    return grouped


def normalize(path: str) -> str:
    return path.strip().replace("\\", "/").removeprefix("./")


def route_path(path: str) -> str | None:
    """Return 'all', 'docs', or a root package name for one changed path."""
    if path == "Cargo.lock" or path.endswith("/Cargo.toml") or path == "Cargo.toml":
        return "all"
    if path.startswith(".github/"):
        return "all"
    for crate_prefix, package in PRODUCT_CRATE_ROWS.items():
        if path.startswith(crate_prefix + "/"):
            return package
    if path.startswith("crates/"):
        return "all"
    if path.startswith("docs/") or path.startswith(".changes/") or path.endswith(".md"):
        return "docs"
    return "all"


def route(paths: list[str]) -> dict[str, object]:
    grouped = rows_by_package()
    if grouped is None:
        print(
            "warning: feature-configuration projection unreadable; failing wide",
            file=sys.stderr,
        )
        return {"mode": "all", "rows": []}
    all_rows = [row for rows in grouped.values() for row in rows]
    saw_docs_only = bool(paths)
    selected: list[str] = []
    fail_wide = False
    for path in paths:
        verdict = route_path(normalize(path))
        if verdict == "all":
            fail_wide = True
            saw_docs_only = False
        elif verdict == "docs":
            pass
        else:
            saw_docs_only = False
            for row in grouped.get(verdict, []):
                if row not in selected:
                    selected.append(row)
    if fail_wide:
        return {"mode": "all", "rows": all_rows}
    if saw_docs_only:
        return {"mode": "rows", "rows": []}
    return {"mode": "rows", "rows": selected}


def main() -> int:
    paths = list(sys.argv[1:])
    stdin_lines = [line.strip() for line in sys.stdin.read().splitlines()]
    paths.extend(line for line in stdin_lines if line)
    print(json.dumps(route(paths)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
