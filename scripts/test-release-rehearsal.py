#!/usr/bin/env python3
"""Fail-closed tests for the release-rehearsal characterization."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
REHEARSAL_PATH = ROOT / "scripts/release-rehearsal.py"
SPEC = importlib.util.spec_from_file_location("release_rehearsal", REHEARSAL_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load release rehearsal harness")
REHEARSAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REHEARSAL)

REQUIRED_PHASES = (
    "release_identity",
    "candidate_package_set",
    "shared_prerequisites",
    "publisher_state_machine",
    "docs_and_support_identity",
    "manifest_and_assets",
    "authorization_boundary",
    "workflow_graph_permissions",
)


class TestReleaseRehearsal(unittest.TestCase):
    def test_characterization_cannot_report_complete(self) -> None:
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")

        self.assertEqual(receipt["schema_version"], "1.0")
        self.assertTrue(
            receipt["subject_lockfile_digest"].startswith("sha256:v1:")
        )
        self.assertTrue(
            receipt["subject_topology_digest"].startswith("sha256:v1:")
        )
        self.assertNotEqual(receipt["aggregate_status"], "Complete")

        proof = receipt["zero_mutation_proof"]
        self.assertTrue(proof)
        self.assertFalse(any(proof.values()))

        phases = receipt["phases"]
        self.assertEqual(set(phases), set(REQUIRED_PHASES))
        for phase_name in REHEARSAL.CHARACTERIZATION_PHASES:
            self.assertNotEqual(
                phases[phase_name],
                "Complete",
                "characterization-only phases cannot manufacture completion",
            )

        assets = receipt.get("manifest_and_assets")
        if phases["manifest_and_assets"] == "Complete":
            self.assertIsInstance(assets, dict)
            self.assertEqual(
                assets["fixture_matrix"],
                "scripts/test-final-packaged-surface.py",
            )

        workflow = receipt.get("workflow_graph_permissions")
        if phases["workflow_graph_permissions"] == "Complete":
            self.assertIsInstance(workflow, dict)
            self.assertIn(workflow["mode"], {"yaml", "text"})
            self.assertTrue(workflow["top_level_read_scoped"])
            self.assertTrue(workflow["top_level_write_scoped"])
            self.assertTrue(workflow["github_release_scoped"])
            self.assertTrue(workflow["authorized_namespace_mode"])

        authorization = receipt.get("authorization_boundary")
        self.assertEqual(phases["authorization_boundary"], "Incomplete")
        self.assertIsInstance(authorization, dict)
        self.assertEqual(authorization["named_release"], "v0.2.0")
        self.assertFalse(authorization["token_present"])

        identity = receipt.get("release_identity")
        docs = receipt.get("docs_and_support_identity")
        if phases["docs_and_support_identity"] == "Complete":
            self.assertIsInstance(docs, dict)
            self.assertTrue(docs["release_record"].endswith(f"/{identity['version']}.md"))
            self.assertTrue(
                docs["github_note"].endswith(f"/github/{identity['tag']}.md")
            )
            self.assertEqual(
                docs["history_check"],
                "scripts/generate-changie-history.py --check",
            )

        packages = receipt.get("candidate_package_set")
        if phases["candidate_package_set"] == "Complete":
            self.assertIsInstance(packages, dict)
            rows = packages["rows"]
            self.assertEqual(len(rows), 10)
            identity_version = identity["version"]
            for row in rows:
                self.assertEqual(row["version"], identity_version)
                self.assertTrue(row["sha256"].startswith("sha256:"))
                self.assertGreater(row["size_bytes"], 0)

        machine = receipt.get("publisher_state_machine")
        if phases["publisher_state_machine"] == "Complete":
            self.assertIsInstance(machine, dict)
            self.assertEqual(
                machine["fixture_matrix"],
                "scripts/test-release-topology-publisher.py",
            )

        shared = receipt.get("shared_prerequisites")
        if phases["shared_prerequisites"] == "Complete":
            self.assertIsInstance(shared, list)
            self.assertEqual(len(shared), 3)
            for row in shared:
                self.assertEqual(row["state"], "already_published_exact")
                self.assertTrue(row["registry_checksum"].startswith("sha256:"))

        self.assertIsInstance(identity, dict)
        self.assertEqual(identity["schema"], "cargo-allow.release-identity.v1")
        self.assertEqual(identity["result"] if "result" in identity else "validated", "validated")
        self.assertTrue(identity["version"])
        self.assertTrue(identity["tag"].startswith("v"))
        self.assertIn(identity["channel"], {"stable", "release_candidate"})
        self.assertIsInstance(identity["github_prerelease"], bool)
        if identity["channel"] == "release_candidate":
            self.assertTrue(identity["github_prerelease"])
            self.assertIsNotNone(identity["rc_ordinal"])
        else:
            self.assertFalse(identity["github_prerelease"])
            self.assertIsNone(identity["rc_ordinal"])

    def test_arbitrary_nonexistent_commit_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt(
                "0123456789abcdef0123456789abcdef01234567"
            )

    def test_option_like_commit_is_rejected_before_git(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.resolve_commit("--help")

    def test_receipt_output_rejects_symlink_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target.json"
            target.write_text("sentinel", encoding="utf-8")
            output = root / "receipt.json"
            output.symlink_to(target)

            with self.assertRaises(OSError):
                REHEARSAL._write_receipt(output, "{}")
            self.assertEqual(target.read_text(encoding="utf-8"), "sentinel")

    def test_registry_token_presence_is_non_clean(self) -> None:
        old_value = os.environ.get("CARGO_REGISTRY_TOKEN")
        try:
            os.environ["CARGO_REGISTRY_TOKEN"] = "synthetic-secret"
            self.assertEqual(
                REHEARSAL.run_phase_authorization_boundary({}),
                "InstrumentFailure",
            )
        finally:
            if old_value is None:
                os.environ.pop("CARGO_REGISTRY_TOKEN", None)
            else:
                os.environ["CARGO_REGISTRY_TOKEN"] = old_value

    def test_cli_exits_nonzero_with_machine_readable_characterization(self) -> None:
        result = subprocess.run(
            [sys.executable, str(REHEARSAL_PATH), "--commit", "HEAD"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn('"aggregate_status"', result.stdout)
        self.assertNotIn('"aggregate_status": "Complete"', result.stdout)


class TestRehearsalSubjectBinding(unittest.TestCase):
    """Exercise checkout admission with real Git and no real phase execution."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-subject-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.git("init", "--quiet")
        files = {
            ".gitignore": "target/\n__pycache__/\n",
            "Cargo.lock": "version = 4\n",
            "policy/product-package-topology-v2.toml": "schema_version = '2.0'\n",
            ".github/workflows/release.yml": "name: fixture\n",
        }
        for name, content in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "first subject")
        self.previous = self.git("rev-parse", "HEAD").strip()
        (self.root / "Cargo.lock").write_text("version = 4\n# current\n", encoding="utf-8")
        self.git("add", "Cargo.lock")
        self.git("commit", "--quiet", "-m", "current subject")
        self.head = self.git("rev-parse", "HEAD").strip()
        root_patch = mock.patch.object(REHEARSAL, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.phases = []
        for name in (
            "run_phase_release_identity",
            "run_phase_candidate_package_set",
            "run_phase_shared_prerequisites",
            "run_phase_publisher_state_machine",
            "run_phase_docs_and_support",
            "run_phase_manifest_and_assets",
            "run_phase_authorization_boundary",
            "run_phase_workflow_graph_permissions",
        ):
            patch = mock.patch.object(REHEARSAL, name, return_value="Incomplete")
            self.phases.append(patch.start())
            self.addCleanup(patch.stop)

    def git(self, *args: str) -> str:
        return subprocess.run(
            [
                "git", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false",
                "-c", f"core.hooksPath={self.root / 'no-hooks'}",
                "-c", "user.name=Rehearsal fixture",
                "-c", "user.email=rehearsal@example.invalid", *args,
            ],
            cwd=self.root, capture_output=True, text=True, timeout=15, check=True,
        ).stdout

    def require_no_phases(self) -> None:
        for phase in self.phases:
            phase.assert_not_called()

    def test_clean_head_and_ignored_artifacts_preserve_characterization(self) -> None:
        for name in (
            "target/cargo-allow/prior.json", "crates/fixture/target/debug/output",
            "__pycache__/fixture.pyc", "scripts/__pycache__/fixture.pyc",
        ):
            artifact = self.root / name
            artifact.parent.mkdir(parents=True, exist_ok=True)
            artifact.write_text("{}", encoding="utf-8")
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertEqual(receipt["aggregate_status"], "Incomplete")
        self.assertTrue(all(value is False for value in receipt["zero_mutation_proof"].values()))
        for phase in self.phases:
            phase.assert_called_once()

    def test_existing_foreign_commit_is_rejected_before_phases(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt(self.previous)
        self.require_no_phases()

    def test_unstaged_selected_source_is_rejected_before_phases(self) -> None:
        for name in (
            "Cargo.lock", "policy/product-package-topology-v2.toml",
            ".github/workflows/release.yml",
        ):
            with self.subTest(path=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_bytes(original + b"# uncommitted\n")
                try:
                    with self.assertRaises(ValueError):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    self.require_no_phases()
                finally:
                    path.write_bytes(original)

    def test_staged_source_is_rejected_before_phases(self) -> None:
        (self.root / "Cargo.lock").write_text("staged change\n", encoding="utf-8")
        self.git("add", "Cargo.lock")
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_index_flags_are_rejected_without_mutating_the_index(self) -> None:
        source = self.root / "Cargo.lock"
        original = source.read_bytes()
        for flag in ("assume-unchanged", "skip-worktree"):
            for changed in (False, True):
                with self.subTest(flag=flag, changed=changed):
                    self.git("update-index", f"--{flag}", "Cargo.lock")
                    if changed:
                        source.write_bytes(original + b"# hidden change\n")
                    # Prove this is the status blind spot, not ordinary dirt.
                    self.assertEqual(self.git("status", "--porcelain"), "")
                    index = (self.root / ".git/index").read_bytes()
                    try:
                        with self.assertRaisesRegex(ValueError, "index flags"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
                        self.require_no_phases()
                        self.assertEqual((self.root / ".git/index").read_bytes(), index)
                    finally:
                        source.write_bytes(original)
                        self.git("update-index", f"--no-{flag}", "Cargo.lock")

    def add_untracked_source(self) -> None:
        path = self.root / ".changes/untracked.md"
        path.parent.mkdir(exist_ok=True)
        path.write_text("untracked release input\n", encoding="utf-8")

    def test_untracked_phase_input_is_rejected_before_phases(self) -> None:
        self.git("config", "status.showUntrackedFiles", "no")
        self.add_untracked_source()
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_ignored_phase_input_is_rejected_before_phases(self) -> None:
        self.add_untracked_source()
        (self.root / ".git/info/exclude").write_text(".changes/*.md\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_external_ignore_cannot_hide_phase_input(self) -> None:
        self.add_untracked_source()
        exclude = self.root / "target/fixture-excludes"
        exclude.parent.mkdir()
        exclude.write_text(".changes/*.md\n", encoding="utf-8")
        self.git("config", "core.excludesFile", str(exclude))
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_ignored_artifact_name_requires_the_exact_directory(self) -> None:
        exclude = self.root / ".git/info/exclude"
        for name in ("target", "target-lookalike.txt", "source with spaces.py"):
            with self.subTest(path=name):
                path = self.root / name
                path.write_text("source, not an artifact directory\n", encoding="utf-8")
                exclude.write_text(f"/{name}\n", encoding="utf-8")
                try:
                    with self.assertRaises(ValueError):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    self.require_no_phases()
                finally:
                    path.unlink()

    def test_staged_rename_is_rejected_before_phases(self) -> None:
        self.git("mv", "Cargo.lock", "renamed lock")
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_status_inspection_failure_is_rejected_without_raw_output(self) -> None:
        run = subprocess.run

        def fail_status(command, **kwargs):
            if "status" in command:
                return subprocess.CompletedProcess(command, 128, b"", b"secret-canary")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_status):
            with self.assertRaisesRegex(ValueError, "could not inspect") as caught:
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertNotIn("secret-canary", str(caught.exception))
        self.require_no_phases()

    def test_index_inspection_failures_reach_cli_without_a_receipt(self) -> None:
        run = subprocess.run
        output = self.root / "target/rejected.json"
        for failure in (
            subprocess.CompletedProcess([], 128, b"secret-canary", b"secret-canary"),
            OSError("fixture Git unavailable"),
            subprocess.TimeoutExpired("git ls-files", 15),
        ):
            with self.subTest(failure=type(failure).__name__):
                def fail_index(command, **kwargs):
                    if "ls-files" in command:
                        if isinstance(failure, Exception):
                            raise failure
                        return failure
                    return run(command, **kwargs)

                stderr = io.StringIO()
                stdout = io.StringIO()
                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_index), \
                     mock.patch.object(sys, "argv", [
                         str(REHEARSAL_PATH), "--commit", "HEAD", "--output", str(output),
                     ]), contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout):
                    self.assertEqual(REHEARSAL.main(), 2)
                self.assertFalse(output.exists())
                self.assertEqual(stdout.getvalue(), "")
                self.assertIn("instrumentation failed", stderr.getvalue())
                self.assertNotIn("secret-canary", stderr.getvalue())
                self.require_no_phases()

    def test_head_movement_during_admission_stops_before_phases(self) -> None:
        run = subprocess.run

        def move_head_after_status(command, **kwargs):
            result = run(command, **kwargs)
            if "status" in command:
                self.git("commit", "--quiet", "--allow-empty", "-m", "moved during check")
            return result

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=move_head_after_status):
            with self.assertRaisesRegex(ValueError, "HEAD moved"):
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_dirty_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt):
            (self.root / "Cargo.lock").write_text("changed by phase\n", encoding="utf-8")
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_hidden_source_after_phases_cannot_return_a_receipt(self) -> None:
        source = self.root / "Cargo.lock"
        original = source.read_bytes()
        for flag in ("assume-unchanged", "skip-worktree"):
            with self.subTest(flag=flag):
                def change_source(receipt):
                    self.git("update-index", f"--{flag}", "Cargo.lock")
                    source.write_bytes(original + b"# hidden phase change\n")
                    return "Incomplete"

                self.phases[0].side_effect = change_source
                try:
                    with self.assertRaisesRegex(ValueError, "index flags"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                finally:
                    source.write_bytes(original)
                    self.git("update-index", f"--no-{flag}", "Cargo.lock")

    def test_untracked_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt):
            self.add_untracked_source()
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_staged_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt):
            (self.root / "Cargo.lock").write_text("staged by phase\n", encoding="utf-8")
            self.git("add", "Cargo.lock")
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_clean_commit_movement_after_phases_cannot_return_a_receipt(self) -> None:
        def change_head(receipt):
            self.git("commit", "--quiet", "--allow-empty", "-m", "moved subject")
            return "Incomplete"

        self.phases[0].side_effect = change_head
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_rejected_subject_cli_writes_no_receipt(self) -> None:
        output = self.root / "target/rejected.json"
        stderr = io.StringIO()
        stdout = io.StringIO()
        with mock.patch.object(sys, "argv", [
            str(REHEARSAL_PATH), "--commit", self.previous, "--output", str(output),
        ]), contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout):
            self.assertEqual(REHEARSAL.main(), 2)
        self.assertFalse(output.exists())
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("instrumentation failed", stderr.getvalue())
        self.require_no_phases()


if __name__ == "__main__":
    unittest.main()
