#!/usr/bin/env python3
"""Adversarial characterization for candidate-harness owned directories."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = ROOT / "scripts" / "candidate-harness-owned-dir.py"


def run(*args: str, expect: int = 0) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        [sys.executable, str(TOOL), *args], capture_output=True, text=True, check=False
    )
    if result.returncode != expect:
        raise SystemExit(
            f"expected exit {expect}, got {result.returncode}: {args!r}\n{result.stdout}{result.stderr}"
        )
    return result


def reject(*args: str) -> None:
    result = subprocess.run(
        [sys.executable, str(TOOL), *args], capture_output=True, text=True, check=False
    )
    if result.returncode == 0:
        raise SystemExit(f"unsafe operation unexpectedly succeeded: {args!r}")


with tempfile.TemporaryDirectory(prefix="cargo-allow-owned-dir-test.") as temporary:
    root = Path(temporary).resolve()
    allocation = json.loads(
        run("allocate", "--root", str(root), "--purpose", "positive").stdout
    )
    owned = Path(allocation["path"])
    (owned / "payload").write_text("owned\n", encoding="utf-8")
    reject(
        "remove", "--root", str(root), "--path", str(owned),
        "--purpose", "positive", "--token", "wrong",
    )
    if not (owned / "payload").is_file():
        raise SystemExit("marker mismatch removed owned payload")
    if __import__("shutil").rmtree.avoids_symlink_attacks:
        run(
            "remove", "--root", str(root), "--path", str(owned),
            "--purpose", "positive", "--token", allocation["token"],
        )
        if owned.exists():
            raise SystemExit("matching marker did not remove exact owned directory")
    else:
        reject(
            "remove", "--root", str(root), "--path", str(owned),
            "--purpose", "positive", "--token", allocation["token"],
        )
        if not (owned / "payload").is_file():
            raise SystemExit("unsupported platform did not fail closed")

    preexisting = root / "preexisting"
    preexisting.mkdir()
    sentinel = preexisting / "sentinel"
    sentinel.write_text("sentinel\n", encoding="utf-8")
    reject("allocate", "--root", str(root), "--purpose", "preexisting", "--durable")
    if sentinel.read_text(encoding="utf-8") != "sentinel\n":
        raise SystemExit("pre-existing sentinel changed")

    reject("remove", "--root", str(root), "--path", str(root), "--purpose", "root", "--token", "x")
    reject("remove", "--root", str(root), "--path", str(root.parent), "--purpose", "ancestor", "--token", "x")
    nested = preexisting / "nested"
    nested.mkdir()
    reject("remove", "--root", str(root), "--path", str(nested), "--purpose", "nested", "--token", "x")
    if hasattr(os, "symlink"):
        alias = root / "alias"
        try:
            alias.symlink_to(preexisting, target_is_directory=True)
        except OSError:
            pass
        else:
            reject("remove", "--root", str(root), "--path", str(alias), "--purpose", "alias", "--token", "x")
            if sentinel.read_text(encoding="utf-8") != "sentinel\n":
                raise SystemExit("symlink negative changed target sentinel")

    snapshot = json.loads(
        run("snapshot", "--root", str(root), "--repository", str(ROOT), "--purpose", "snapshot-auth").stdout
    )
    snap_path = snapshot["path"]
    run("verify", "--root", str(root), "--path", snap_path, "--purpose", "snapshot-auth",
        "--token", snapshot["token"], "--git-head", snapshot["git_head"], "--repository", str(ROOT))
    reject("verify", "--root", str(root), "--path", snap_path, "--purpose", "snapshot-auth",
           "--token", snapshot["token"], "--git-head", "forged", "--repository", str(ROOT))
    reject("verify", "--root", str(root), "--path", str(root), "--purpose", "snapshot-auth",
           "--token", snapshot["token"], "--git-head", snapshot["git_head"], "--repository", str(ROOT))
    if __import__("shutil").rmtree.avoids_symlink_attacks:
        run("remove", "--root", str(root), "--path", snap_path,
            "--purpose", "snapshot-auth", "--token", snapshot["token"])

    if __import__("shutil").rmtree.avoids_symlink_attacks:
        for script in ("exact-candidate-package-set.sh", "source-candidate-smoke.sh"):
            result = subprocess.run(
                ["bash", str(ROOT / "scripts" / script)],
                cwd=ROOT,
                env={**os.environ, "CANDIDATE_HARNESS_SNAPSHOT_PROBE": "1", "CANDIDATE_HARNESS_TEST_INJECTION": "1", "CANDIDATE_HARNESS_TEST_ROOT": str(root), "CANDIDATE_HARNESS_TOKEN": "forged", "CANDIDATE_HARNESS_ROOT": str(ROOT), "CANDIDATE_HARNESS_OUTPUT_ROOT": str(ROOT), "CANDIDATE_HARNESS_GIT_HEAD": "forged"},
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode != 0 or "disposable snapshot ok" not in result.stdout:
                raise SystemExit(f"snapshot probe failed for {script}:\n{result.stdout}{result.stderr}")

print("ok candidate harness owned-directory containment")
