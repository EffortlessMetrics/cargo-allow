#!/usr/bin/env python3
"""Generate a review inventory of production filesystem mutation surfaces (#3692).

This is a one-shot branch helper. The generated report is an investigation
artifact, not the final checked authority and not evidence that a surface is
safe merely because it was found.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/release/writer-surface-inventory.generated.md"

PATTERNS = [
    ("direct_fs_write", re.compile(r"\b(?:std::)?fs::write\s*\(")),
    ("file_create", re.compile(r"\bFile::create\s*\(")),
    ("open_options", re.compile(r"\bOpenOptions::new\s*\(")),
    ("write_all", re.compile(r"\.write_all\s*\(")),
    ("rename", re.compile(r"\b(?:std::)?fs::rename\s*\(")),
    ("remove_file", re.compile(r"\b(?:std::)?fs::remove_file\s*\(")),
    ("remove_dir_all", re.compile(r"\b(?:std::)?fs::remove_dir_all\s*\(")),
    ("create_dir_all", re.compile(r"\b(?:std::)?fs::create_dir_all\s*\(")),
    ("apply_single_target", re.compile(r"\bapply_single_target\s*\(")),
    ("emit_text", re.compile(r"\bemit_text\s*\(")),
    ("write_file", re.compile(r"\bwrite_file\s*\(")),
    ("atomic_write", re.compile(r"\batomic_write\w*\s*\(")),
    ("persist", re.compile(r"\b(?:persist|save|store|flush)_\w*\s*\(")),
]

EXCLUDED_PARTS = {"tests", "fixtures"}
EXCLUDED_SUFFIXES = ("_tests.rs", "test.rs", "tests.rs")


def is_test_like(path: Path) -> bool:
    relative = path.relative_to(ROOT)
    return (
        any(part in EXCLUDED_PARTS for part in relative.parts)
        or relative.name.endswith(EXCLUDED_SUFFIXES)
        or relative.name.startswith("test_")
    )


def main() -> None:
    rows: list[tuple[str, int, str, str]] = []
    for path in sorted((ROOT / "crates").glob("*/src/**/*.rs")):
        if is_test_like(path):
            continue
        relative = path.relative_to(ROOT).as_posix()
        text = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(text.splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("//"):
                continue
            for kind, pattern in PATTERNS:
                if pattern.search(line):
                    snippet = stripped.replace("|", "\\|")
                    rows.append((relative, line_number, kind, snippet))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Generated writer-surface reconnaissance",
        "",
        "> Investigation output for #3692. Every row still requires an explicit",
        "> semantic owner and disposition. Regenerate from the exact source head;",
        "> do not treat presence in this report as approval.",
        "",
        f"Rows: **{len(rows)}**",
        "",
        "| Path | Line | Primitive/wrapper | Source marker |",
        "| --- | ---: | --- | --- |",
    ]
    for path, line_number, kind, snippet in rows:
        lines.append(f"| `{path}` | {line_number} | `{kind}` | `{snippet}` |")
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"writer-inventory: wrote {len(rows)} rows to {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
