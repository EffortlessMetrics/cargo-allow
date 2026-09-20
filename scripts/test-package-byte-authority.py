#!/usr/bin/env python3
"""Package-byte authority verifier and cross-producer proof (#4304).

The candidate package authority accepts a `.crate` only when its
`.cargo_vcs_info.json` binds the exact frozen Git head and explicitly records a
clean worktree. The same verifier is used by the production package-set script
and by these real Cargo packaging controls.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from io import BytesIO
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LIFECYCLE = REPO / "scripts" / "candidate-harness-owned-dir.py"
PROBE_CRATE = "effortless-repo-snapshot"
VCS_MEMBER_SUFFIX = "/.cargo_vcs_info.json"


def run(arguments, cwd=None):
    completed = subprocess.run(
        arguments, cwd=cwd, check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


def harness(*arguments):
    payload = run([sys.executable, str(LIFECYCLE), *arguments], cwd=str(REPO))
    return json.loads(payload)


def harness_nout(*arguments):
    """Harness commands that prove by exit status and print no JSON."""
    run([sys.executable, str(LIFECYCLE), *arguments], cwd=str(REPO))


def cleanup_owned(root, path, purpose, token, worktree=False):
    """Release a harness-owned directory, then dispose its bytes."""
    command = "worktree-remove" if worktree else "remove"
    try:
        harness_nout(
            command,
            "--root",
            str(root),
            "--path",
            str(path),
            "--purpose",
            purpose,
            "--token",
            token,
        )
    except subprocess.CalledProcessError:
        if worktree or sys.platform != "win32":
            raise
        shutil.rmtree(path, ignore_errors=True)


def archive_members(archive):
    with tarfile.open(archive, mode="r:gz") as bundle:
        return {
            member.name: bundle.extractfile(member).read()
            for member in bundle.getmembers()
            if member.isfile()
        }


def rewrite_vcs_dirty(archive, prefix, output, dirty):
    """Rewrite only git.dirty in a real Cargo-produced archive."""
    member_name = f"{prefix}/.cargo_vcs_info.json"
    rewritten = 0
    with tarfile.open(archive, mode="r:gz") as source, tarfile.open(
        output, mode="w:gz"
    ) as target:
        for member in source.getmembers():
            data = None
            if member.isfile():
                payload = source.extractfile(member)
                if payload is None:
                    raise AssertionError(f"could not read archive member {member.name}")
                data = payload.read()
            if member.name == member_name:
                info = json.loads(data.decode("utf-8"))
                git = info.get("git")
                if not isinstance(git, dict):
                    raise AssertionError("real Cargo VCS metadata lacks its git object")
                git["dirty"] = dirty
                data = (json.dumps(info, sort_keys=True) + "\n").encode("utf-8")
                member.size = len(data)
                rewritten += 1
            target.addfile(member, BytesIO(data) if data is not None else None)
    if rewritten != 1:
        raise AssertionError(
            f"expected one VCS metadata member to rewrite, found {rewritten}"
        )


def rewrite_source_member(archive, prefix, output):
    """Change source bytes while retaining the archive's VCS assertion."""
    member_name = f"{prefix}/src/lib.rs"
    rewritten = 0
    with tarfile.open(archive, mode="r:gz") as source, tarfile.open(
        output, mode="w:gz"
    ) as target:
        for member in source.getmembers():
            data = None
            if member.isfile():
                payload = source.extractfile(member)
                if payload is None:
                    raise AssertionError(f"could not read archive member {member.name}")
                data = payload.read()
            if member.name == member_name:
                data += b"\n// tampered prebuilt source with retained VCS metadata\n"
                member.size = len(data)
                rewritten += 1
            target.addfile(member, BytesIO(data) if data is not None else None)
    if rewritten != 1:
        raise AssertionError(
            f"expected one source member to rewrite, found {rewritten}"
        )


def sole_crate(target_dir):
    crates = sorted(target_dir.glob("*.crate"))
    if len(crates) != 1:
        raise AssertionError(
            f"expected exactly one packaged crate under {target_dir}, found {len(crates)}"
        )
    return crates[0]


def read_vcs_info(archive, prefix):
    """Read the one exact VCS metadata member from a packaged crate."""
    member_name = f"{prefix}/.cargo_vcs_info.json"
    try:
        with tarfile.open(archive, mode="r:gz") as bundle:
            members = [
                member
                for member in bundle.getmembers()
                if member.isfile() and member.name == member_name
            ]
            if len(members) != 1:
                raise ValueError(
                    "expected exactly one .cargo_vcs_info.json "
                    f"at {member_name}, found {len(members)}"
                )
            payload = bundle.extractfile(members[0])
            if payload is None:
                raise ValueError("could not read .cargo_vcs_info.json")
            info = json.loads(payload.read().decode("utf-8"))
    except (OSError, tarfile.TarError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"unreadable .cargo_vcs_info.json: {error}") from error
    if not isinstance(info, dict):
        raise ValueError(".cargo_vcs_info.json must be a JSON object")
    return info


def verify_archive(archive, prefix, expected_head):
    """Require the exact Git subject and Cargo's canonical clean posture."""
    info = read_vcs_info(archive, prefix)
    git = info.get("git")
    if not isinstance(git, dict):
        raise ValueError(".cargo_vcs_info.json must contain a git object")

    sha = git.get("sha1")
    if not isinstance(sha, str) or not sha:
        raise ValueError("missing git.sha1 in .cargo_vcs_info.json")
    if sha != expected_head:
        raise ValueError(f"git.sha1 {sha} does not match frozen subject {expected_head}")

    dirty = git.get("dirty", False)
    if type(dirty) is not bool:
        raise ValueError("git.dirty must be a JSON boolean when present")
    if dirty:
        raise ValueError("git.dirty must be false")

    normalized = dict(info)
    normalized_git = dict(git)
    normalized_git["dirty"] = dirty
    normalized["git"] = normalized_git
    return normalized

def verifier_process(archive, prefix, expected_head):
    return subprocess.run(
        [
            sys.executable,
            str(Path(__file__).resolve()),
            "verify-archive",
            "--archive",
            str(archive),
            "--prefix",
            prefix,
            "--expected-head",
            expected_head,
        ],
        cwd=str(REPO),
        capture_output=True,
        text=True,
        check=False,
    )


def verifier_cli(argv):
    parser = argparse.ArgumentParser(
        prog="test-package-byte-authority.py verify-archive"
    )
    parser.add_argument("--archive", required=True)
    parser.add_argument("--prefix", required=True)
    parser.add_argument("--expected-head", required=True)
    args = parser.parse_args(argv)
    try:
        info = verify_archive(args.archive, args.prefix, args.expected_head)
    except ValueError as error:
        print(f"package-byte-authority: error: {error}", file=sys.stderr)
        return 1
    print(
        json.dumps(
            {"sha1": info["git"]["sha1"], "dirty": info["git"]["dirty"]},
            sort_keys=True,
        )
    )
    return 0


class PackageByteAuthorityTests(unittest.TestCase):
    def test_snapshot_packaging_omits_vcs_info_and_is_rejected(self):
        """Sensitivity control: bare snapshots cannot prove the Git subject."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        snapshot = harness(
            "snapshot",
            "--root",
            str(root),
            "--repository",
            str(REPO),
            "--purpose",
            "byte-authority-bare",
        )
        self.addCleanup(
            cleanup_owned,
            root,
            snapshot["path"],
            "byte-authority-bare",
            snapshot["token"],
        )
        run(
            [
                "cargo",
                "package",
                "-p",
                PROBE_CRATE,
                "--locked",
                "--allow-dirty",
                "--no-verify",
            ],
            cwd=snapshot["path"],
        )
        archive = sole_crate(Path(snapshot["path"]) / "target" / "package")
        members = archive_members(archive)
        vcs = [name for name in members if name.endswith(VCS_MEMBER_SUFFIX)]
        self.assertEqual(
            vcs, [], f"bare snapshot packaging must omit vcs info, found {vcs}"
        )
        rejected = verifier_process(
            archive, archive.name.removesuffix(".crate"), head
        )
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("expected exactly one .cargo_vcs_info.json", rejected.stderr)

    def test_worktree_packaging_carries_exact_clean_head(self):
        """Clean Git-backed packaging is accepted for the exact frozen head."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        worktree = harness(
            "worktree",
            "--root",
            str(root),
            "--repository",
            str(REPO),
            "--purpose",
            "byte-authority-backed",
            "--head",
            head,
        )
        self.addCleanup(
            cleanup_owned,
            root,
            worktree["path"],
            "byte-authority-backed",
            worktree["token"],
            True,
        )
        self.assertEqual(worktree["git_head"], head)
        run(
            [
                "cargo",
                "package",
                "-p",
                PROBE_CRATE,
                "--locked",
                "--no-verify",
            ],
            cwd=worktree["path"],
        )
        archive = sole_crate(Path(worktree["path"]) / "target" / "package")
        prefix = archive.name.removesuffix(".crate")
        info = read_vcs_info(archive, prefix)
        self.assertEqual(info.get("git", {}).get("sha1"), head)
        self.assertNotIn(
            "dirty",
            info.get("git", {}),
            "Cargo canonically omits dirty for a clean package",
        )
        accepted = verifier_process(archive, prefix, head)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)

        explicit_false = root / "explicit-dirty-false.crate"
        rewrite_vcs_dirty(archive, prefix, explicit_false, False)
        accepted_false = verifier_process(explicit_false, prefix, head)
        self.assertEqual(accepted_false.returncode, 0, accepted_false.stderr)

        for label, malformed in (("string", "false"), ("null", None), ("integer", 0)):
            with self.subTest(dirty_posture=label):
                malformed_archive = root / f"malformed-dirty-{label}.crate"
                rewrite_vcs_dirty(archive, prefix, malformed_archive, malformed)
                rejected = verifier_process(malformed_archive, prefix, head)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn(
                    "git.dirty must be a JSON boolean when present",
                    rejected.stderr,
                )

        # Embedded VCS metadata is self-reported archive content. Prove that a
        # non-VCS source mutation can retain an apparently clean exact SHA, then
        # prove the exact-set authority refuses the prebuilt route entirely.
        tampered = root / "tampered-source-with-clean-vcs.crate"
        rewrite_source_member(archive, prefix, tampered)
        metadata_only = verifier_process(tampered, prefix, head)
        self.assertEqual(
            metadata_only.returncode,
            0,
            "the discriminator requires intact VCS metadata on changed source bytes",
        )
        prebuilt = root / "prebuilt-input"
        prebuilt.mkdir()
        shutil.copyfile(tampered, prebuilt / archive.name)
        rejected_prebuilt = subprocess.run(
            ["bash", str(REPO / "scripts" / "exact-candidate-package-set.sh")],
            cwd=str(REPO),
            env={
                **os.environ,
                "SKIP_PACKAGE": "1",
                "PACKAGE_INPUT_DIR": str(prebuilt),
            },
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(rejected_prebuilt.returncode, 0)
        self.assertIn(
            "SKIP_PACKAGE=1 is snapshot-probe-only", rejected_prebuilt.stderr
        )

    def test_dirty_worktree_package_is_rejected(self):
        """Same SHA plus dirty tracked bytes must never satisfy authority."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        worktree = harness(
            "worktree",
            "--root",
            str(root),
            "--repository",
            str(REPO),
            "--purpose",
            "byte-authority-dirty",
            "--head",
            head,
        )
        self.addCleanup(
            cleanup_owned,
            root,
            worktree["path"],
            "byte-authority-dirty",
            worktree["token"],
            True,
        )
        source = (
            Path(worktree["path"])
            / "crates"
            / PROBE_CRATE
            / "src"
            / "lib.rs"
        )
        source.write_text(
            source.read_text(encoding="utf-8")
            + "\n// package-byte authority dirty negative control\n",
            encoding="utf-8",
            newline="\n",
        )
        run(
            [
                "cargo",
                "package",
                "-p",
                PROBE_CRATE,
                "--locked",
                "--allow-dirty",
                "--no-verify",
            ],
            cwd=worktree["path"],
        )
        archive = sole_crate(Path(worktree["path"]) / "target" / "package")
        prefix = archive.name.removesuffix(".crate")
        info = read_vcs_info(archive, prefix)
        self.assertEqual(info.get("git", {}).get("sha1"), head)
        self.assertIs(info.get("git", {}).get("dirty"), True)
        rejected = verifier_process(archive, prefix, head)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("git.dirty must be false", rejected.stderr)

    def test_divergence_is_vcs_info_only(self):
        """The VCS file is the entire producer divergence, never source."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        snapshot = harness(
            "snapshot",
            "--root",
            str(root),
            "--repository",
            str(REPO),
            "--purpose",
            "byte-authority-compare-bare",
        )
        self.addCleanup(
            cleanup_owned,
            root,
            snapshot["path"],
            "byte-authority-compare-bare",
            snapshot["token"],
        )
        worktree = harness(
            "worktree",
            "--root",
            str(root),
            "--repository",
            str(REPO),
            "--purpose",
            "byte-authority-compare-backed",
            "--head",
            head,
        )
        removed = False
        try:
            run(
                [
                    "cargo",
                    "package",
                    "-p",
                    PROBE_CRATE,
                    "--locked",
                    "--allow-dirty",
                    "--no-verify",
                ],
                cwd=snapshot["path"],
            )
            run(
                [
                    "cargo",
                    "package",
                    "-p",
                    PROBE_CRATE,
                    "--locked",
                    "--allow-dirty",
                    "--no-verify",
                ],
                cwd=worktree["path"],
            )
            bare = archive_members(
                sole_crate(Path(snapshot["path"]) / "target" / "package")
            )
            backed = archive_members(
                sole_crate(Path(worktree["path"]) / "target" / "package")
            )
            bare_names = set(bare)
            backed_names = set(backed)
            self.assertEqual(
                backed_names - bare_names,
                {
                    name
                    for name in backed_names
                    if name.endswith(VCS_MEMBER_SUFFIX)
                },
                "the git-backed archive must add only the vcs file",
            )
            self.assertEqual(
                bare_names - backed_names,
                set(),
                "the bare archive must not add any file of its own",
            )
            for name in bare_names & backed_names:
                self.assertEqual(
                    bare[name],
                    backed[name],
                    f"shared member differs between producers: {name}",
                )
        finally:
            try:
                harness_nout(
                    "worktree-remove",
                    "--root",
                    str(root),
                    "--path",
                    worktree["path"],
                    "--purpose",
                    "byte-authority-compare-backed",
                    "--token",
                    worktree["token"],
                )
                removed = True
            except subprocess.CalledProcessError:
                pass
        if not removed:
            self.fail("worktree cleanup failed; registration may be stale")
        registrations = run(
            ["git", "-C", str(REPO), "worktree", "list", "--porcelain"]
        )
        self.assertNotIn(
            worktree["path"],
            registrations,
            "cleanup must leave no stale worktree registration",
        )


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "verify-archive":
        raise SystemExit(verifier_cli(sys.argv[2:]))
    unittest.main(verbosity=2)
