#!/usr/bin/env python3
"""Adversarial characterization for candidate-harness owned directories."""

from __future__ import annotations

import json
import errno
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = ROOT / "scripts" / "candidate-harness-owned-dir.py"
SPEC = importlib.util.spec_from_file_location("candidate_harness_owned_dir", TOOL)
assert SPEC and SPEC.loader
LIFECYCLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LIFECYCLE)


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

        # Deterministic child substitution between validation and deletion.
        race = json.loads(run("allocate", "--root", str(root), "--purpose", "race").stdout)
        race_path = Path(race["path"])
        (race_path / "sentinel").write_text("preserve\n", encoding="utf-8")
        original_rmtree = LIFECYCLE.shutil.rmtree
        original_safe = getattr(original_rmtree, "avoids_symlink_attacks", False)
        def substitute_child(path: Path, *args, **kwargs):
            moved = path.with_name(path.name + ".foreign")
            os.rename(path, moved)
            try:
                path.symlink_to(moved, target_is_directory=True)
            except OSError:
                os.rename(moved, path)
                raise
            return original_rmtree(path, *args, **kwargs)
        substitute_child.avoids_symlink_attacks = original_safe
        LIFECYCLE.shutil.rmtree = substitute_child
        try:
            try:
                LIFECYCLE.remove(root, race_path, "race", race["token"])
            except OSError as error:
                if not original_safe or error.errno not in {errno.EINVAL, errno.ENOTDIR, errno.ELOOP, errno.EPERM}:
                    raise
            except SystemExit:
                pass
            else:
                raise SystemExit("TOCTOU child substitution unexpectedly succeeded")
        finally:
            LIFECYCLE.shutil.rmtree = original_rmtree
        foreign = race_path.with_name(race_path.name + ".foreign")
        if not (foreign / "sentinel").is_file():
            raise SystemExit("TOCTOU substitution damaged foreign sentinel")
        if race_path.exists() and not race_path.is_symlink():
            raise SystemExit("TOCTOU substitution unexpectedly replaced race path")
        if race_path.is_symlink():
            race_path.unlink()
        original_rmtree(foreign)
    else:
        # Windows and other unsupported platforms must refuse matching
        # removal; the payload remains untouched.
        reject(
            "remove", "--root", str(root), "--path", str(owned),
            "--purpose", "positive", "--token", allocation["token"],
        )
        if not (owned / "payload").is_file():
            raise SystemExit("unsupported platform did not preserve payload")

    # CI pre-creates the canonical package-set parent.  Allocation owns only
    # a fresh child and cleanup must leave that parent (and its inputs) intact.
    package_parent = root / "exact-candidate-package-set"
    package_parent.mkdir()
    (package_parent / "packages").mkdir()
    parent_sentinel = package_parent / "packages" / "candidate.crate"
    parent_sentinel.write_bytes(b"precreated\n")
    child = json.loads(
        run("allocate", "--root", str(package_parent), "--purpose", "exact-candidate-package-set").stdout
    )
    child_path = Path(child["path"])
    (child_path / "scratch").write_text("child\n", encoding="utf-8")
    if __import__("shutil").rmtree.avoids_symlink_attacks:
        run("remove", "--root", str(package_parent), "--path", str(child_path),
            "--purpose", "exact-candidate-package-set", "--token", child["token"])
        if child_path.exists():
            raise SystemExit("matching marker did not remove package-set child")
    else:
        reject("remove", "--root", str(package_parent), "--path", str(child_path),
               "--purpose", "exact-candidate-package-set", "--token", child["token"])
    if not package_parent.is_dir() or not parent_sentinel.is_file():
        raise SystemExit("parent cleanup removed pre-created package-set input")

    source_parent = root / "source-candidate-smoke"
    source_parent.mkdir()
    source_sentinel = source_parent / "source-candidate-smoke.receipt.json"
    source_sentinel.write_text("precreated\n", encoding="utf-8")
    source_child = json.loads(
        run("allocate", "--root", str(source_parent), "--purpose", "source-candidate-smoke").stdout
    )
    source_child_path = Path(source_child["path"])
    if __import__("shutil").rmtree.avoids_symlink_attacks:
        run("remove", "--root", str(source_parent), "--path", str(source_child_path),
            "--purpose", "source-candidate-smoke", "--token", source_child["token"])
        if source_child_path.exists():
            raise SystemExit("matching marker did not remove source-candidate child")
    else:
        reject("remove", "--root", str(source_parent), "--path", str(source_child_path),
               "--purpose", "source-candidate-smoke", "--token", source_child["token"])
    if not source_parent.is_dir() or source_sentinel.read_text(encoding="utf-8") != "precreated\n":
        raise SystemExit("source-candidate parent cleanup removed pre-created output")

    # Target aliases must be rejected before a harness can mkdir through them.
    run("validate-target", "--repository", str(ROOT), "--path", str(ROOT / "target"))
    alias_repo = root / "target-alias-repo"
    alias_repo.mkdir()
    alias_target = root / "target-alias-external"
    alias_target.mkdir()
    alias_sentinel = alias_target / "sentinel"
    alias_sentinel.write_text("preserve\n", encoding="utf-8")
    if hasattr(os, "symlink"):
        target_alias = alias_repo / "target"
        try:
            target_alias.symlink_to(alias_target, target_is_directory=True)
        except OSError:
            pass
        else:
            reject("validate-target", "--repository", str(alias_repo), "--path", str(target_alias))
            if alias_sentinel.read_text(encoding="utf-8") != "preserve\n":
                raise SystemExit("target alias validation changed external sentinel")

    # Restore collision characterization: the shell restore contract must
    # refuse to overwrite a destination that appeared while the stash exists.
    collision_stash = root / "collision.stash"
    collision_destination = root / "collision.destination"
    collision_stash.mkdir()
    (collision_stash / "source").write_text("stash\n", encoding="utf-8")
    collision_destination.mkdir()
    (collision_destination / "sentinel").write_text("preserve\n", encoding="utf-8")
    receipt = collision_destination / "source-candidate-smoke.receipt.json"
    try:
        LIFECYCLE.restore(collision_stash, collision_destination)
    except SystemExit:
        pass
    else:
        raise SystemExit("restore collision unexpectedly overwrote destination")
    if not (collision_stash / "source").is_file() or not (collision_destination / "sentinel").is_file():
        raise SystemExit("restore collision damaged stash or destination sentinel")
    if receipt.exists():
        raise SystemExit("restore collision left a misleading receipt")

    # Harness entrypoints reject broad/overlapping test roots before snapshot
    # allocation, even when probe mode is disabled.
    bad_roots = [ROOT, ROOT / "target", ROOT.parent, root.parent, Path(ROOT.anchor)]
    ancestor_sentinel = root.parent / ".cargo-allow-3509-root-sentinel"
    ancestor_sentinel.write_text("preserve\n", encoding="utf-8")
    for script in ("exact-candidate-package-set.sh", "source-candidate-smoke.sh"):
        for bad_root in bad_roots:
            result = subprocess.run(
                ["bash", str(ROOT / "scripts" / script)], cwd=ROOT,
                env={**os.environ, "CANDIDATE_HARNESS_TEST_ROOT": str(bad_root)},
                capture_output=True, text=True, check=False,
            )
            if result.returncode == 0:
                raise SystemExit(f"{script} accepted unsafe test root {bad_root}")
        if (ROOT / "target" / "exact-candidate-package-set").exists():
            raise SystemExit("unsafe root validation created candidate output")
    if ancestor_sentinel.read_text(encoding="utf-8") != "preserve\n":
        raise SystemExit("unsafe ancestor root validation changed sentinel")
    ancestor_sentinel.unlink()

    # Inject a destination appearance after the helper's preflight check.
    rename = LIFECYCLE.os.rename
    injected = {"done": False}
    def race_rename(source: Path, destination: Path) -> None:
        if not injected["done"] and destination == root / "rename-race-destination":
            injected["done"] = True
            destination.mkdir()
            (destination / "sentinel").write_text("preserve\n", encoding="utf-8")
        return rename(source, destination)
    LIFECYCLE.os.rename = race_rename
    rename_stash = root / "rename-race-stash"
    rename_destination = root / "rename-race-destination"
    rename_stash.mkdir()
    (rename_stash / "source").write_text("stash\n", encoding="utf-8")
    try:
        try:
            LIFECYCLE.restore(rename_stash, rename_destination)
        except (FileExistsError, SystemExit):
            pass
        else:
            raise SystemExit("restore rename race unexpectedly succeeded")
    finally:
        LIFECYCLE.os.rename = rename
    if not (rename_stash / "source").is_file() or not (rename_destination / "sentinel").is_file():
        raise SystemExit("restore rename race damaged stash or destination")
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

        # CI supplies the checkout root as PACKAGE_INPUT_DIR.  The package
        # harness must stage its crate inputs into the snapshot-local target,
        # with digest/provenance evidence, before entering the snapshot.
        package_input = root / "ci-checkout"
        package_dir = package_input / "target" / "package-candidate-smoke" / "packages"
        package_dir.mkdir(parents=True)
        (package_dir / "candidate.crate").write_bytes(b"candidate-input\n")
        result = subprocess.run(
            ["bash", str(ROOT / "scripts" / "exact-candidate-package-set.sh")],
            cwd=ROOT,
            env={**os.environ, "SKIP_PACKAGE": "1", "PACKAGE_INPUT_DIR": str(package_input),
                 "CANDIDATE_HARNESS_SNAPSHOT_PROBE": "1", "CANDIDATE_HARNESS_TEST_INJECTION": "1",
                 "CANDIDATE_HARNESS_TEST_ROOT": str(root)},
            capture_output=True, text=True, check=False,
        )
        if result.returncode != 0 or "disposable snapshot ok" not in result.stdout:
            raise SystemExit(f"checkout-root package staging probe failed:\n{result.stdout}{result.stderr}")

print("ok candidate harness owned-directory containment")
