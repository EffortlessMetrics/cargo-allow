#!/usr/bin/env python3
"""Allocate and remove marker-owned release-harness directories."""

from __future__ import annotations

import argparse
import json
import os
import secrets
import shutil
import stat
import subprocess
import tarfile
import tempfile
from io import BytesIO
from pathlib import Path

MARKER = ".candidate-harness-owner.json"


def fail(message: str) -> None:
    raise SystemExit(f"candidate-harness-owned-dir: {message}")


def canonical_existing(path: Path, label: str) -> Path:
    if not str(path).strip() or not path.exists() or path.is_symlink():
        fail(f"{label} must be an existing non-symlink directory: {path}")
    resolved = path.resolve(strict=True)
    if not resolved.is_dir():
        fail(f"{label} must be a directory: {resolved}")
    return resolved


def validate_test_root(root: Path, repository: Path) -> Path:
    allowed = canonical_existing(root, "test root")
    repo = canonical_existing(repository, "repository")
    target = repo / "target"
    if allowed == repo or repo in allowed.parents:
        fail(f"test root overlaps repository: {allowed}")
    if target.exists() and (allowed == target or target in allowed.parents or allowed in target.parents):
        fail(f"test root overlaps repository target: {allowed}")
    if allowed.parent == allowed:
        fail(f"test root must not be filesystem root: {allowed}")
    return allowed


def validate_purpose(purpose: str) -> None:
    if not purpose or any(character not in "abcdefghijklmnopqrstuvwxyz0123456789-" for character in purpose):
        fail(f"invalid purpose {purpose!r}")


def write_marker(directory: Path, purpose: str, token: str, **metadata: str) -> None:
    marker = directory / MARKER
    marker.write_text(json.dumps({"purpose": purpose, "token": token, **metadata}) + "\n", encoding="utf-8")
    os.chmod(directory, stat.S_IRWXU)
    os.chmod(marker, stat.S_IRUSR | stat.S_IWUSR)


def allocate(root: Path, purpose: str, durable: bool) -> tuple[Path, str]:
    validate_purpose(purpose)
    allowed = canonical_existing(root, "allowed root")
    directory = allowed / purpose if durable else Path(tempfile.mkdtemp(prefix=f"{purpose}.", dir=allowed))
    if durable:
        try:
            directory.mkdir(mode=0o700)
        except FileExistsError:
            fail(f"refusing pre-existing durable directory: {directory}")
    token = secrets.token_hex(32)
    write_marker(directory, purpose, token)
    return directory.resolve(strict=True), token


def load_marker(directory: Path) -> dict[str, object]:
    try:
        marker = directory / MARKER
        if marker.is_symlink():
            fail(f"refusing symlink marker: {marker}")
        value = json.loads(marker.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"missing or malformed ownership marker for {directory}: {error}")
    if not isinstance(value, dict):
        fail(f"ownership marker must be an object: {directory / MARKER}")
    return value


def remove(root: Path, directory: Path, purpose: str, token: str) -> None:
    if not shutil.rmtree.avoids_symlink_attacks:
        fail("platform does not provide symlink-safe recursive deletion")
    validate_purpose(purpose)
    allowed = canonical_existing(root, "allowed root")
    if not directory.exists() or directory.is_symlink():
        fail(f"owned directory must exist and must not be a symlink: {directory}")
    resolved = directory.resolve(strict=True)
    if resolved.parent != allowed:
        fail(f"owned directory must be a direct child of {allowed}: {resolved}")
    if resolved.name != purpose and not resolved.name.startswith(f"{purpose}."):
        fail(f"owned directory name does not match purpose {purpose!r}: {resolved.name}")
    marker = load_marker(resolved)
    if marker.get("purpose") != purpose or marker.get("token") != token:
        fail(f"ownership marker mismatch for {resolved}")
    if resolved.resolve(strict=True).parent != canonical_existing(root, "allowed root"):
        fail(f"owned directory parent changed before deletion: {resolved}")
    shutil.rmtree(resolved)


def restore(stash: Path, destination: Path) -> None:
    if not stash.exists() or stash.is_symlink() or not stash.is_dir():
        fail(f"restore stash must be an existing non-symlink directory: {stash}")
    if destination.exists() or destination.is_symlink():
        fail(f"refusing to overwrite existing restore destination: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    os.rename(stash, destination)


def snapshot(root: Path, repository: Path, purpose: str) -> tuple[Path, str, str]:
    repo = canonical_existing(repository, "repository")
    directory, token = allocate(root, purpose, False)
    try:
        head = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            check=True, capture_output=True, text=True,
        ).stdout.strip()
        # Keep tar bytes on a pipe.  The helper's stdout is a JSON protocol
        # consumed by the shell harness; binary archive data must never leak
        # into that stream.
        archive_result = subprocess.run(
            ["git", "-C", str(repo), "archive", "--format=tar", head],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        archive = archive_result.stdout
        with tarfile.open(fileobj=BytesIO(archive), mode="r:") as bundle:
            for member in bundle.getmembers():
                target = (directory / member.name).resolve()
                if directory not in target.parents:
                    fail(f"archive member escapes snapshot: {member.name}")
            bundle.extractall(directory, filter="data")
        write_marker(directory, purpose, token, git_head=head, repository=str(repo))
        return directory, token, head
    except Exception:
        if directory.exists():
            if shutil.rmtree.avoids_symlink_attacks:
                remove(root, directory, purpose, token)
        raise


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    allocate_parser = subparsers.add_parser("allocate")
    allocate_parser.add_argument("--root", type=Path, required=True)
    allocate_parser.add_argument("--purpose", required=True)
    allocate_parser.add_argument("--durable", action="store_true")
    remove_parser = subparsers.add_parser("remove")
    remove_parser.add_argument("--root", type=Path, required=True)
    remove_parser.add_argument("--path", type=Path, required=True)
    remove_parser.add_argument("--purpose", required=True)
    remove_parser.add_argument("--token", required=True)
    snapshot_parser = subparsers.add_parser("snapshot")
    snapshot_parser.add_argument("--root", type=Path, required=True)
    snapshot_parser.add_argument("--repository", type=Path, required=True)
    snapshot_parser.add_argument("--purpose", required=True)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--root", type=Path, required=True)
    verify_parser.add_argument("--path", type=Path, required=True)
    verify_parser.add_argument("--purpose", required=True)
    verify_parser.add_argument("--token", required=True)
    verify_parser.add_argument("--git-head")
    verify_parser.add_argument("--repository", type=Path)
    restore_parser = subparsers.add_parser("restore")
    restore_parser.add_argument("--stash", type=Path, required=True)
    restore_parser.add_argument("--destination", type=Path, required=True)
    root_parser = subparsers.add_parser("validate-test-root")
    root_parser.add_argument("--root", type=Path, required=True)
    root_parser.add_argument("--repository", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "allocate":
        directory, token = allocate(args.root, args.purpose, args.durable)
        print(json.dumps({"path": str(directory), "purpose": args.purpose, "token": token}))
        return 0
    if args.command == "snapshot":
        directory, token, head = snapshot(args.root, args.repository, args.purpose)
        print(json.dumps({"path": str(directory), "purpose": args.purpose, "token": token, "git_head": head}))
        return 0
    if args.command == "verify":
        allowed = canonical_existing(args.root, "allowed root")
        directory = canonical_existing(args.path, "owned directory")
        if directory.parent != allowed:
            fail(f"owned directory must be a direct child of {allowed}: {directory}")
        marker = load_marker(directory)
        if marker.get("purpose") != args.purpose or marker.get("token") != args.token:
            fail(f"ownership marker mismatch for {directory}")
        if args.git_head is not None and marker.get("git_head") != args.git_head:
            fail(f"snapshot git head mismatch for {directory}")
        if args.repository is not None:
            repository = canonical_existing(args.repository, "repository")
            if marker.get("repository") != str(repository):
                fail(f"snapshot repository mismatch for {directory}")
        return 0
    if args.command == "restore":
        restore(args.stash, args.destination)
        return 0
    if args.command == "validate-test-root":
        print(validate_test_root(args.root, args.repository))
        return 0
    remove(args.root, args.path, args.purpose, args.token)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
