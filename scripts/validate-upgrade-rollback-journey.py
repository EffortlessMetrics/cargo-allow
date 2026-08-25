#!/usr/bin/env python3
"""Validate the bounded exact installed upgrade/rollback receipt."""
import argparse
import hashlib
import json
import re
from pathlib import Path


def fail(message):
    raise SystemExit(f"exact-upgrade-rollback: {message}")


def load(path):
    try:
        value = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    if not isinstance(value, dict):
        fail("receipt must be an object")
    return value


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    args = parser.parse_args()
    receipt = load(args.receipt)
    schema = load(args.schema)
    if receipt.get("schema_id") != schema["properties"]["schema_id"]["const"]:
        fail("wrong schema id")
    if receipt.get("result") != "Passed":
        fail("receipt is not Passed")
    for key in ("from", "candidate", "rollback", "repository", "steps", "negative_controls"):
        if key not in receipt:
            fail(f"missing {key}")
    if not re.fullmatch(r"cargo-allow 0\.1\.11.*", receipt["from"]["version"]):
        fail("from leg is not 0.1.11")
    if not re.fullmatch(r"cargo-allow 0\.2\.0.*", receipt["candidate"]["version"]):
        fail("candidate leg is not 0.2.0")
    if receipt["rollback"]["binary_sha256"] != receipt["from"]["binary_sha256"]:
        fail("rollback binary identity changed")
    if receipt["repository"]["fixture_sha256"] != digest(args.fixture):
        fail("fixture digest mismatch")
    if receipt["repository"]["unrelated_file_preserved"] is not True:
        fail("unrelated-file control missing")
    if len(receipt["steps"]) < 9:
        fail("three legs are incomplete")
    required = {"old_binary_exact_version", "candidate_binary_exact_version", "checkout_binary_not_used", "unrelated_file_survives_rollback"}
    actual = {row.get("id") for row in receipt["negative_controls"] if row.get("passed") is True}
    if not required <= actual:
        fail("required negative control missing")
    print(f"validated exact upgrade/rollback receipt: {args.receipt}")


if __name__ == "__main__":
    main()
