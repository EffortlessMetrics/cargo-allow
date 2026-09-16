"""Commit one deterministic product-workspace projection and floor lock."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys


ALLOWED_DERIVED_PATHS = {"Cargo.toml", "Cargo.lock"}
PROJECTION_SCHEMA = "cargo-allow.direct-floor-product-workspace.v1"


def git(*arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments], capture_output=True, text=True, check=False, timeout=30
    )
    if result.returncode:
        raise ValueError(f"git {arguments[0]} failed: {result.stderr.strip()}")
    return result.stdout


def _digest(path: Path) -> str:
    raw = path.read_bytes().replace(b"\r", b"")
    return "sha256:v1:" + hashlib.sha256(raw).hexdigest()


def _projection(path: Path) -> dict[str, object]:
    try:
        projection = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"floor workspace projection is unreadable: {error}") from error
    if projection.get("schema_id") != PROJECTION_SCHEMA:
        raise ValueError("floor workspace projection has an unsupported schema")
    if projection.get("projected_manifest_digest") != _digest(Path("Cargo.toml")):
        raise ValueError("floor workspace projection does not bind the current Cargo.toml")
    for field in ("certified_member_paths", "execution_member_paths"):
        values = projection.get(field)
        if not isinstance(values, list) or not values or not all(
            isinstance(value, str) and value for value in values
        ):
            raise ValueError(f"floor workspace projection has invalid {field}")
    return projection


def _admitted_changes(status: str) -> list[str]:
    changes: list[str] = []
    for record in status.split("\0"):
        if not record:
            continue
        if len(record) < 4:
            raise ValueError("floor derivation received malformed Git status")
        disposition = record[:2]
        path = record[3:]
        if disposition != " M" or path not in ALLOWED_DERIVED_PATHS:
            raise ValueError(
                "floor derivation permits only unstaged Cargo.toml and Cargo.lock changes"
            )
        changes.append(path)
    if "Cargo.toml" not in changes:
        raise ValueError("floor derivation requires the product workspace projection")
    return sorted(set(changes))


def derive(source: str, projection_path: Path) -> dict[str, object]:
    if git("rev-parse", "HEAD").strip() != source:
        raise ValueError("floor source HEAD changed before derivation")
    if git("branch", "--show-current").strip():
        raise ValueError("floor derivation requires a detached checkout")
    flags = git("ls-files", "-v", "-z").split("\0")
    if any(entry and (entry[0].islower() or entry[0] == "S") for entry in flags):
        raise ValueError("floor derivation rejects hidden index entries")
    ignored = git("ls-files", "--others", "--ignored", "--exclude-standard", "-z")
    if any(path and not path.startswith("target/") for path in ignored.split("\0")):
        raise ValueError("floor derivation rejects ignored files outside root target")

    projection = _projection(projection_path)
    changes = _admitted_changes(
        git("status", "--porcelain=v1", "--untracked-files=all", "-z")
    )
    git("add", "--", *changes)
    git(
        "-c",
        "core.hooksPath=",
        "-c",
        "commit.gpgSign=false",
        "-c",
        "user.name=cargo-allow floor proof",
        "-c",
        "user.email=floor-proof@example.invalid",
        "commit",
        "-m",
        "chore(proof): derive product-scoped direct-floor subject",
    )

    derived = git("rev-parse", "HEAD").strip()
    if git("rev-list", "--parents", "-n", "1", "HEAD").split() != [derived, source]:
        raise ValueError("derived floor commit must have exactly the source parent")
    diff_paths = {
        path
        for path in git("diff", "--name-only", "-z", source, derived).split("\0")
        if path
    }
    if diff_paths != set(changes):
        raise ValueError("derived floor commit changed files outside its admitted write set")
    if git("status", "--porcelain=v1", "--untracked-files=all", "-z"):
        raise ValueError("derived floor checkout is not clean")

    return {
        "source_commit": source,
        "derived_commit": derived,
        "derived_tree": git("rev-parse", "HEAD^{tree}").strip(),
        "derived_paths": changes,
        "projection_schema_id": projection["schema_id"],
        "projected_manifest_digest": projection["projected_manifest_digest"],
        "certified_member_paths": projection["certified_member_paths"],
        "execution_member_paths": projection["execution_member_paths"],
    }


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(
            "usage: floor_source_identity.py <source-commit> <workspace-projection.json>",
            file=sys.stderr,
        )
        return 1
    try:
        print(json.dumps(derive(argv[1], Path(argv[2])), indent=1))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"proof-direct-floors: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
