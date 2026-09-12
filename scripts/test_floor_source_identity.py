"""Real Git controls for the floor collector's derived source subject."""

import argparse
import importlib.util
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "floor_source_identity", Path(__file__).with_name("floor_source_identity.py"),
)
SUBJECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUBJECT)
REHEARSAL = None


class FloorSourceIdentityTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="floor-source-identity-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        previous = Path.cwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, previous)
        self.git("init", "-q")
        self.git("config", "user.name", "Floor fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "commit.gpgSign", "false")
        self.git("config", "core.hooksPath", "")
        Path(".gitignore").write_text("target/\nignored.rs\n")
        Path("Cargo.lock").write_text("original lock\n")
        Path("source.rs").write_text("original source\n")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture source")
        self.git("checkout", "--detach", "-q")
        self.source = self.git("rev-parse", "HEAD").strip()
        Path("target/floor-proof").mkdir(parents=True)
        Path("target/floor-proof/scratch.json").write_text("{}")

    def git(self, *arguments):
        return subprocess.run(
            ["git", *arguments], check=True, capture_output=True, text=True,
        ).stdout

    def test_derived_lock_is_clean_and_preserves_source_parent(self):
        Path("Cargo.lock").write_text("floor lock\n")
        if REHEARSAL is not None:
            REHEARSAL.ROOT = self.root
            scratch = ("execution-identity.json", "floors-selection.json", "floors.json", "pin-failures.json")
            for name in scratch:
                Path(name).write_text("{}")
            with self.assertRaises(ValueError):
                REHEARSAL.require_clean_checkout(self.source)
            for name in scratch:
                Path(name).rename(Path("target/floor-proof") / name)
        result = SUBJECT.derive(self.source)
        if REHEARSAL is not None:
            REHEARSAL.require_clean_checkout(result["derived_commit"])
        self.assertEqual(result["source_commit"], self.source)
        self.assertNotEqual(result["derived_commit"], self.source)
        self.assertEqual(self.git("rev-parse", "HEAD^").strip(), self.source)
        self.assertEqual(self.git("diff", "--name-only", self.source), "Cargo.lock\n")
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual(Path("source.rs").read_text(), "original source\n")

    def test_unchanged_lock_reuses_source_without_empty_commit(self):
        self.assertEqual(SUBJECT.derive(self.source)["derived_commit"], self.source)

    def test_rejects_source_changes_before_commit(self):
        for kind in ("tracked", "staged", "untracked", "ignored", "hidden", "skip"):
            with self.subTest(kind=kind):
                if kind in ("tracked", "staged", "hidden", "skip"):
                    Path("source.rs").write_text("changed\n")
                if kind == "staged":
                    self.git("add", "source.rs")
                if kind in ("untracked", "ignored"):
                    Path("new.rs" if kind == "untracked" else "ignored.rs").write_text("new\n")
                if kind in ("hidden", "skip"):
                    self.git("update-index", "--assume-unchanged" if kind == "hidden" else "--skip-worktree", "source.rs")
                with self.assertRaises(ValueError):
                    SUBJECT.derive(self.source)
                self.assertEqual(self.git("rev-parse", "HEAD").strip(), self.source)
                self.git("update-index", "--no-assume-unchanged", "source.rs")
                self.git("update-index", "--no-skip-worktree", "source.rs")
                self.git("restore", "--staged", "--worktree", "source.rs")
                for name in ("new.rs", "ignored.rs"):
                    Path(name).unlink(missing_ok=True)

    def test_rejects_attached_checkout(self):
        self.git("switch", "-c", "fixture-attached")
        with self.assertRaises(ValueError):
            SUBJECT.derive(self.source)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--rehearsal-script", type=Path)
    options, remaining = parser.parse_known_args()
    if options.rehearsal_script:
        rehearsal_spec = importlib.util.spec_from_file_location(
            "floor_rehearsal", options.rehearsal_script.resolve(),
        )
        REHEARSAL = importlib.util.module_from_spec(rehearsal_spec)
        rehearsal_spec.loader.exec_module(REHEARSAL)
        if not callable(getattr(REHEARSAL, "require_clean_checkout", None)):
            raise SystemExit("selected rehearsal does not implement strict admission")
    unittest.main(argv=[sys.argv[0], *remaining])
