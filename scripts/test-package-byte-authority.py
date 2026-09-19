#!/usr/bin/env python3
"""Cross-producer package-byte authority proof (#4304).

`cargo package` embeds `.cargo_vcs_info.json` if and only if the packaged
directory observes git metadata. A bare `git archive` snapshot therefore
freezes different immutable `.crate` bytes than the git-backed publish path
for the same source tree. This proof packages one real workspace crate both
ways from the same head and requires:

- bare-snapshot packaging omits `.cargo_vcs_info.json` (sensitivity: the
  oracle sees the defect it guards against);
- worktree packaging carries `.cargo_vcs_info.json` with the exact head SHA;
- both archives agree byte-for-byte on every other member (the vcs file is
  the entire divergence, never a source difference);
- harness cleanup leaves no stale worktree registration behind.

Synthetic subjects only in the sense that no registry, tag, or publish is
touched; the cargo invocations themselves are real, because only real
packaging exhibits the mechanism. CI economy: one smallest crate, no
workspace-wide packaging, no verification builds.
"""

import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
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
    payload = run(
        [sys.executable, str(LIFECYCLE), *arguments], cwd=str(REPO)
    )
    return json.loads(payload)


def harness_nout(*arguments):
    """Harness commands that prove by exit status and print no JSON."""
    run([sys.executable, str(LIFECYCLE), *arguments], cwd=str(REPO))


def cleanup_owned(root, path, purpose, token, worktree=False):
    """Release a harness-owned directory, then dispose its bytes.

    Worktree removal is git-based and works everywhere. Plain removal
    needs symlink-safe recursive deletion, which Windows lacks; there the
    enclosing root disposal still removes the bytes, while Linux CI
    exercises the harness-owned path.
    """
    command = "worktree-remove" if worktree else "remove"
    try:
        harness_nout(
            command,
            "--root", str(root),
            "--path", str(path),
            "--purpose", purpose,
            "--token", token,
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


def sole_crate(target_dir):
    crates = sorted(target_dir.glob("*.crate"))
    if len(crates) != 1:
        raise AssertionError(
            f"expected exactly one packaged crate under {target_dir}, found {len(crates)}"
        )
    return crates[0]


class PackageByteAuthorityTests(unittest.TestCase):
    def test_snapshot_packaging_omits_vcs_info(self):
        """Sensitivity control: bare snapshots cannot carry the Git subject."""
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        snapshot = harness(
            "snapshot",
            "--root", str(root),
            "--repository", str(REPO),
            "--purpose", "byte-authority-bare",
        )
        self.addCleanup(
            cleanup_owned, root, snapshot["path"], "byte-authority-bare",
            snapshot["token"],
        )
        run(
            [
                "cargo", "package", "-p", PROBE_CRATE,
                "--locked", "--allow-dirty", "--no-verify",
            ],
            cwd=snapshot["path"],
        )
        members = archive_members(
            sole_crate(Path(snapshot["path"]) / "target" / "package")
        )
        vcs = [name for name in members if name.endswith(VCS_MEMBER_SUFFIX)]
        self.assertEqual(
            vcs, [], f"bare snapshot packaging must omit vcs info, found {vcs}"
        )

    def test_worktree_packaging_carries_exact_head(self):
        """Authority: git-backed packaging embeds the exact head SHA."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        worktree = harness(
            "worktree",
            "--root", str(root),
            "--repository", str(REPO),
            "--purpose", "byte-authority-backed",
            "--head", head,
        )
        self.addCleanup(
            cleanup_owned, root, worktree["path"], "byte-authority-backed",
            worktree["token"], True,
        )
        self.assertEqual(worktree["git_head"], head)
        run(
            [
                "cargo", "package", "-p", PROBE_CRATE,
                "--locked", "--allow-dirty", "--no-verify",
            ],
            cwd=worktree["path"],
        )
        members = archive_members(
            sole_crate(Path(worktree["path"]) / "target" / "package")
        )
        vcs = [name for name in members if name.endswith(VCS_MEMBER_SUFFIX)]
        self.assertEqual(
            len(vcs), 1, f"git-backed packaging must carry exactly one vcs file, found {vcs}"
        )
        info = json.loads(members[vcs[0]].decode("utf-8"))
        self.assertEqual(
            info.get("git", {}).get("sha1"), head,
            "vcs info must name the exact packaged head",
        )

    def test_divergence_is_vcs_info_only(self):
        """The vcs file is the entire producer divergence, never source."""
        head = run(["git", "-C", str(REPO), "rev-parse", "HEAD"])
        root = Path(tempfile.mkdtemp(prefix="byte-authority-"))
        self.addCleanup(shutil.rmtree, root, True)
        snapshot = harness(
            "snapshot",
            "--root", str(root),
            "--repository", str(REPO),
            "--purpose", "byte-authority-compare-bare",
        )
        self.addCleanup(
            cleanup_owned, root, snapshot["path"], "byte-authority-compare-bare",
            snapshot["token"],
        )
        worktree = harness(
            "worktree",
            "--root", str(root),
            "--repository", str(REPO),
            "--purpose", "byte-authority-compare-backed",
            "--head", head,
        )
        removed = False
        try:
            run(
                [
                    "cargo", "package", "-p", PROBE_CRATE,
                    "--locked", "--allow-dirty", "--no-verify",
                ],
                cwd=snapshot["path"],
            )
            run(
                [
                    "cargo", "package", "-p", PROBE_CRATE,
                    "--locked", "--allow-dirty", "--no-verify",
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
                {name for name in backed_names if name.endswith(VCS_MEMBER_SUFFIX)},
                "the git-backed archive must add only the vcs file",
            )
            self.assertEqual(
                bare_names - backed_names, set(),
                "the bare archive must not add any file of its own",
            )
            for name in bare_names & backed_names:
                self.assertEqual(
                    bare[name], backed[name],
                    f"shared member differs between producers: {name}",
                )
        finally:
            try:
                harness_nout(
                    "worktree-remove",
                    "--root", str(root),
                    "--path", worktree["path"],
                    "--purpose", "byte-authority-compare-backed",
                    "--token", worktree["token"],
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
            worktree["path"], registrations,
            "cleanup must leave no stale worktree registration",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
