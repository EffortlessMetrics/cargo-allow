#!/usr/bin/env python3
"""Union a preserved Changie version body into a freshly batched one (#3725).

`changie batch 0.2.0` rewrites `.changes/0.2.0.md` from the unreleased
fragments. The prospective final file that existed before the cut carried
authored bullets that no fragment reproduces, so a plain batch silently
drops them. This script merges the two by kind: the batched bullets keep
their order, then the preserved bullets for the same kind follow, with
exact-duplicate bullets removed.

Usage:
    python scripts/apply-changie-0-2-0-union.py <batched.md> <preserved.md> <out.md>
"""

from __future__ import annotations

import sys
from pathlib import Path

KIND_PREFIX = "### "


def split_sections(text: str) -> tuple[str, list[tuple[str, list[str]]]]:
    """Return (heading, [(kind, [bullet_block, ...])]) preserving order."""
    lines = text.splitlines()
    heading = ""
    sections: list[tuple[str, list[str]]] = []
    current_kind: str | None = None
    current_blocks: list[list[str]] = []

    def flush() -> None:
        if current_kind is not None:
            sections.append((current_kind, ["\n".join(b) for b in current_blocks]))

    for line in lines:
        if line.startswith(KIND_PREFIX):
            flush()
            current_kind = line[len(KIND_PREFIX):].strip()
            current_blocks = []
            continue
        if current_kind is None:
            if line.strip():
                heading = line.strip()
            continue
        if line.startswith("- "):
            current_blocks.append([line])
        elif current_blocks and line.strip():
            current_blocks[-1].append(line)
    flush()
    return heading, sections


def normalize(block: str) -> str:
    return " ".join(block.split())


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    batched_path, preserved_path, out_path = (Path(p) for p in sys.argv[1:])
    heading, batched = split_sections(batched_path.read_text(encoding="utf-8"))
    _, preserved = split_sections(preserved_path.read_text(encoding="utf-8"))

    preserved_by_kind: dict[str, list[str]] = {}
    for kind, blocks in preserved:
        preserved_by_kind.setdefault(kind, []).extend(blocks)

    out: list[str] = [heading, ""]
    seen: set[str] = set()
    for kind, blocks in batched:
        merged: list[str] = []
        for block in blocks:
            key = normalize(block)
            if key and key not in seen:
                seen.add(key)
                merged.append(block)
        for block in preserved_by_kind.pop(kind, []):
            key = normalize(block)
            if key and key not in seen:
                seen.add(key)
                merged.append(block)
        if not merged:
            continue
        out.append(f"{KIND_PREFIX}{kind}")
        out.append("")
        for block in merged:
            out.append(block)
            out.append("")
    for kind, blocks in preserved_by_kind.items():
        if not blocks:
            continue
        out.append(f"{KIND_PREFIX}{kind}")
        out.append("")
        for block in blocks:
            out.append(block)
            out.append("")

    out_path.write_text("\n".join(out).rstrip("\n") + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
