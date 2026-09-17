#!/usr/bin/env python3
"""Run full-workspace contracts before deriving a product-scoped floor subject.

This is an internal producer preflight, not direct-floor compatibility evidence.
Its admitted test names become the projected phase's explicit skip filters only
when the original, clean, locked workspace actually passes exactly those tests.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


TESTS = (
    "ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly",
    "package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly",
    "product_package_topology_tests::current_v2_authorities_drive_package_candidate",
    "publish_order_validation_tests::publish_order_covers_all_workspace_members",
    "release_prep_tests::published_release_versions_match_workspace",
)
TARGET_DIR = "target/floor-proof/source-workspace-target"
CLAIM = (
    "Original locked-workspace topology contracts only; not direct-floor proof. "
    "These tests remain enforced before the product workspace is projected."
)


def digest(raw: bytes) -> str:
    return "sha256:v1:" + hashlib.sha256(raw).hexdigest()


def admit_output(output: str, status: int) -> None:
    """A zero exit alone, ignored test, or empty filter is never enough."""
    if status:
        raise ValueError(f"source-workspace contract command failed (exit {status})")
    lines = output.splitlines()
    observations = [line for line in lines if line.startswith("test ")
                    and not line.startswith("test result:")]
    expected = [f"test {name} ... ok" for name in TESTS]
    if sorted(observations) != sorted(expected):
        raise ValueError("source-workspace preflight must pass exactly the five named tests")
    if [line for line in lines if line.startswith("running ")] != ["running 5 tests"]:
        raise ValueError("source-workspace preflight must execute one five-test suite")
    summaries = [line for line in lines if line.startswith("test result:")]
    pattern = (r"test result: ok\. 5 passed; 0 failed; 0 ignored; 0 measured; "
               r"[0-9]+ filtered out; finished in [0-9.]+s")
    if len(summaries) != 1 or re.fullmatch(pattern, summaries[0]) is None:
        raise ValueError("source-workspace preflight needs one complete non-ignored pass summary")


def source_identity(expected: str) -> dict[str, str]:
    def git(*arguments: str) -> str:
        result = subprocess.run(["git", *arguments], capture_output=True, text=True, check=False)
        if result.returncode:
            raise ValueError("source-workspace preflight could not observe Git identity")
        return result.stdout

    head = git("rev-parse", "HEAD").strip()
    tree = git("rev-parse", "HEAD^{tree}").strip()
    if head != expected or re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise ValueError("source-workspace preflight source commit mismatch")
    if re.fullmatch(r"[0-9a-f]{40}", tree) is None:
        raise ValueError("source-workspace preflight source tree is malformed")
    if git("status", "--porcelain=v1", "--untracked-files=all", "-z"):
        raise ValueError("source-workspace preflight requires clean source")
    return {
        "source_commit": head,
        "source_tree": tree,
        "manifest_digest": digest(Path("Cargo.toml").read_bytes().replace(b"\r", b"")),
        "lock_digest": digest(Path("Cargo.lock").read_bytes().replace(b"\r", b"")),
    }


def run_preflight(source_commit: str, host_target: str, output: Path) -> dict:
    output.parent.mkdir(parents=True, exist_ok=True)
    # A failed rerun must not leave a prior successful admission in place.
    output.unlink(missing_ok=True)
    before = source_identity(source_commit)
    command = [
        "cargo", "test", "--locked", "--target", host_target,
        "--target-dir", TARGET_DIR, "-p", "cargo-allow", "--bin", "cargo-allow",
        "--", "--format", "pretty", "--color", "never", *TESTS,
    ]
    # Use the same substring matching as libtest --skip, then require exactly
    # TESTS in the observed results. A new name colliding with a skip filter
    # therefore blocks admission instead of silently expanding the exclusion.
    environment = dict(os.environ, RUST_TEST_NOCAPTURE="0")
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                            errors="replace", env=environment, check=False)
    stdout = result.stdout.replace("\r\n", "\n")
    stderr = result.stderr.replace("\r\n", "\n")
    output.with_suffix(".stdout.log").write_text(stdout, encoding="utf-8", newline="\n")
    output.with_suffix(".stderr.log").write_text(stderr, encoding="utf-8", newline="\n")
    print(stdout, end="")
    print(stderr, end="", file=sys.stderr)
    admit_output(stdout, result.returncode)
    if source_identity(source_commit) != before:
        raise ValueError("source-workspace preflight changed its source inputs")
    observation = {
        **before,
        "target": host_target,
        "command": command,
        "tests": list(TESTS),
        "result": "passed",
        "stdout_digest": digest(stdout.encode("utf-8")),
        "stderr_digest": digest(stderr.encode("utf-8")),
        "claim_boundary": CLAIM,
    }
    output.write_text(json.dumps(observation, indent=1) + "\n", encoding="utf-8", newline="\n")
    return observation


def main(arguments: list[str]) -> int:
    if len(arguments) != 4:
        print("usage: floor_workspace_contract.py <source-commit> <host-target> <output.json>",
              file=sys.stderr)
        return 2
    try:
        run_preflight(arguments[1], arguments[2], Path(arguments[3]))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"floor-workspace-contract: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
