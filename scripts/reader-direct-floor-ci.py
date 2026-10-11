#!/usr/bin/env python3
"""Retain an unmodified direct-floor collector run on the temporary proof branch."""

from __future__ import annotations

import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def git(*arguments: str) -> bytes:
    return subprocess.run(["git", *arguments], check=True, capture_output=True).stdout


def main() -> int:
    if sys.version_info < (3, 11):
        raise RuntimeError("the existing floor producer requires Python 3.11 or newer")
    product = os.environ["CI_PROOF_PRODUCT"]
    if product not in {"cargo-allow", "shared", "cargo-intent", "cargo-proof"}:
        raise RuntimeError("unregistered proof product")
    source = git("rev-parse", "HEAD").decode().strip()
    if source != os.environ["GITHUB_SHA"]:
        raise RuntimeError("checkout is not the triggering commit")
    if git("status", "--porcelain=v1", "--untracked-files=all", "-z"):
        raise RuntimeError("proof source must start clean")

    output = Path("target/reader-direct-floor-evidence")
    output.mkdir(parents=True, exist_ok=False)
    receipt = output / f"direct-floor-proof-{product}-v1.json"
    # Bash resolves this relative POSIX path before entering its worktree;
    # a native Windows drive/backslash path would not have POSIX dirname semantics.
    environment = dict(os.environ, CI_PROOF_OUT=receipt.as_posix())
    context = {
        "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_commit": source,
        "source_tree": git("rev-parse", "HEAD^{tree}").decode().strip(),
        "python": sys.version,
        "python_executable": sys.executable,
        "environment": {key: environment.get(key) for key in (
            "CI_PROOF_PRODUCT", "CI_PROOF_CLASSES", "CI_PROOF_ADVISORY",
            "CI_PROOF_MSRV", "RUSTUP_TOOLCHAIN", "RUSTUP_AUTO_INSTALL",
            "PYTHONDONTWRITEBYTECODE", "PYTHONUTF8", "CARGO_INCREMENTAL", "CARGO_PROFILE_DEV_DEBUG",
            "CARGO_PROFILE_TEST_DEBUG", "CARGO_BUILD_JOBS", "GITHUB_RUN_ID",
            "GITHUB_RUN_ATTEMPT", "GITHUB_JOB", "RUNNER_OS", "RUNNER_ARCH",
        )},
        "command": ["bash", "scripts/proof-direct-floors.sh"],
        "producer_sha256": hashlib.sha256(Path("scripts/proof-direct-floors.sh").read_bytes()).hexdigest(),
    }
    (output / "source-commit.raw").write_bytes(git("cat-file", "commit", source))
    (output / "source-tree.txt").write_bytes(git("ls-tree", "-r", "--full-tree", source))
    (output / "execution-context.json").write_text(json.dumps(context, indent=2) + "\n", encoding="utf-8", newline="\n")
    with (output / "collector.stdout.log").open("wb") as stdout, (output / "collector.stderr.log").open("wb") as stderr:
        result = subprocess.run(context["command"], env=environment, stdout=stdout, stderr=stderr, check=False)
    context["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    context["collector_exit_code"] = result.returncode
    context["source_unchanged"] = source == git("rev-parse", "HEAD").decode().strip() and not git("status", "--porcelain=v1", "--untracked-files=all", "-z")
    for stream in ("stdout", "stderr"):
        data = (output / f"collector.{stream}.log").read_bytes()
        context[stream] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        destination = sys.stdout if stream == "stdout" else sys.stderr
        destination.write(data.decode("utf-8", errors="replace"))

    (output / "execution-result.json").write_text(json.dumps(context, indent=2) + "\n", encoding="utf-8", newline="\n")

    # The collector removes its own temporary worktree. Its derived commit
    # remains in this checkout's object store, so retain the exact permitted
    # projection and lock without altering any generated receipt bytes.
    companion = receipt.with_suffix(".selection.md")
    if companion.is_file():
        text = companion.read_text(encoding="utf-8")
        match = re.search(r"^- Executed floor commit: ([0-9a-f]{40})$", text, re.MULTILINE)
        if match is None:
            raise RuntimeError("generated companion lacks an exact floor commit")
        derived = match.group(1)
        if git("rev-list", "--parents", "-n", "1", derived).decode().split() != [derived, source]:
            raise RuntimeError("derived subject does not have exactly the source parent")
        changed = git("diff", "--name-only", "-z", source, derived).decode().split("\0")
        if not set(filter(None, changed)) <= {"Cargo.toml", "Cargo.lock"}:
            raise RuntimeError("derived subject changed an unadmitted path")
        (output / "derived-commit.raw").write_bytes(git("cat-file", "commit", derived))
        (output / "derived-tree.txt").write_bytes(git("ls-tree", "-r", "--full-tree", derived))
        (output / "derived-source.diff").write_bytes(git("diff", "--binary", source, derived))
        for filename in ("Cargo.toml", "Cargo.lock"):
            (output / ("derived-" + filename)).write_bytes(git("show", f"{derived}:{filename}"))
        context["derived_commit"] = derived

    (output / "execution-result.json").write_text(json.dumps(context, indent=2) + "\n", encoding="utf-8", newline="\n")
    if not context["source_unchanged"]:
        raise RuntimeError("collector changed its original source")
    if result.returncode:
        return result.returncode
    data = json.loads(receipt.read_text(encoding="utf-8"))
    if data["product"] != product or data["msrv"] != "1.95" or data["toolchain"] != "1.95.0" or data["target"] != "host:x86_64-unknown-linux-gnu":
        raise RuntimeError("receipt does not match the intended Linux 1.95 execution")
    if not data["rows"] or any(row["result"] != "proven" for row in data["rows"]):
        raise RuntimeError("collector did not prove every selected floor")
    if not companion.is_file():
        raise RuntimeError("collector did not emit the selection companion")
    print(f"Fresh {product} floor receipt: {len(data['rows'])} proven rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
