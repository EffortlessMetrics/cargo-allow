#!/usr/bin/env python3
"""Check the evidence-strength inventory for release and campaign tests."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path
from typing import Any

EVIDENCE_CLASSES = {
    "LexicalProjectionOnly",
    "StructuredShapeValidation",
    "TypedModelValidation",
    "ProductionBehaviorValidation",
    "ExternalObservationValidation",
    "LiveControlReadback",
    "HistoricalFixtureOnly",
    "DeferredWithNamedOwner",
    "UnsupportedOrMisclassified",
}
SCHEMA = "cargo-allow.evidence-surface-inventory.v1"

REQUIRED_FIELDS = {
    "id",
    "owner_issue",
    "semantic_authority",
    "path",
    "subject",
    "producer",
    "consumer",
    "claimed_acceptance_row",
    "assertion_mechanism",
    "evidence_class",
    "disposition",
    "required_stronger_owner",
    "may_satisfy_release_gate",
    "last_reconciled_commit",
    "claim_boundary",
}

# These are the repository's release/campaign projection surfaces. A new file
# matching this contract must be classified explicitly, rather than inheriting
# the strength of its issue title or test name.
CANDIDATE_GLOB = "crates/cargo-allow/tests/*.rs"
CANDIDATE_NAME = re.compile(
    r"(?:campaign|candidate|final|pilot|publication|rc1|release|review|skill|support)"
)
LEXICAL_MARKERS = ("require_contains", "contains(", "read_to_string", "read(")


def load_inventory(path: Path) -> list[dict[str, Any]]:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    rows = data.get("surfaces")
    if not isinstance(rows, list):
        raise ValueError("inventory must contain [[surfaces]] rows")
    return rows


def candidate_paths(root: Path) -> set[str]:
    result: set[str] = set()
    for path in root.glob(CANDIDATE_GLOB):
        if not CANDIDATE_NAME.search(path.name):
            continue
        text = path.read_text(encoding="utf-8")
        if path.name in {"release_manifest_evidence_v2.rs", "release_rehearsal.rs"} or any(
            marker in text for marker in LEXICAL_MARKERS
        ):
            result.add(path.relative_to(root).as_posix())
    return result


def validate(root: Path, inventory_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        rows = load_inventory(inventory_path)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        return [f"inventory unreadable: {error}"]
    data = tomllib.loads(inventory_path.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA:
        errors.append(f"inventory schema must be {SCHEMA}")

    seen_ids: set[str] = set()
    seen_paths: set[str] = set()
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            errors.append(f"row {index} is not a table")
            continue
        missing = sorted(REQUIRED_FIELDS - row.keys())
        if missing:
            errors.append(f"row {index} missing fields: {', '.join(missing)}")
            continue
        row_id = row["id"]
        row_path = row["path"]
        if not isinstance(row["owner_issue"], int) or row["owner_issue"] <= 0:
            errors.append(f"row {row_id} must have a positive integer owner_issue")
        if not isinstance(row_id, str) or not row_id.strip():
            errors.append(f"row {index} has an invalid id")
        elif row_id in seen_ids:
            errors.append(f"duplicate inventory id: {row_id}")
        else:
            seen_ids.add(row_id)
        if not isinstance(row_path, str) or not row_path.strip():
            errors.append(f"row {index} has an invalid path")
        elif row_path in seen_paths:
            errors.append(f"duplicate inventory path: {row_path}")
        else:
            seen_paths.add(row_path)
            if not (root / row_path).is_file():
                errors.append(f"row {row_id} points to missing path: {row_path}")
        if row.get("evidence_class") not in EVIDENCE_CLASSES:
            errors.append(f"row {row_id} has an invalid evidence class")
        if not isinstance(row.get("may_satisfy_release_gate"), bool):
            errors.append(f"row {row_id} must declare may_satisfy_release_gate as boolean")
        for field in REQUIRED_FIELDS - {"owner_issue", "may_satisfy_release_gate"}:
            if field in row and not isinstance(row[field], str):
                errors.append(f"row {row_id} field {field} must be a string")

    expected = candidate_paths(root)
    missing = sorted(expected - seen_paths)
    for path in missing:
        errors.append(f"unclassified load-bearing test: {path}")
    stale = sorted(seen_paths - expected)
    for path in stale:
        errors.append(f"inventory path is not a discovered load-bearing test: {path}")

    for row in rows:
        if not isinstance(row, dict) or "id" not in row:
            continue
        evidence_class = row.get("evidence_class")
        if evidence_class == "LexicalProjectionOnly" and row.get("may_satisfy_release_gate"):
            errors.append(
                f"row {row['id']} cannot use LexicalProjectionOnly as release-gate evidence"
            )
        if evidence_class in {"DeferredWithNamedOwner", "UnsupportedOrMisclassified"}:
            owner = row.get("required_stronger_owner", "")
            if not isinstance(owner, str) or not owner.strip():
                errors.append(f"row {row['id']} needs a stronger owner")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory", type=Path, default=Path("policy/evidence-surface-inventory.toml"))
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    errors = validate(args.root.resolve(), args.inventory.resolve())
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    rows = load_inventory(args.inventory)
    print(f"evidence surface inventory: {len(rows)} classified rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
