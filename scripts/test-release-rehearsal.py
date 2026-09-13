#!/usr/bin/env python3
"""Fail-closed tests for the release-rehearsal characterization."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
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


class TestReceiptOutput(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.sandbox = Path(directory.name).resolve()
        self.root = self.sandbox / "repo"
        self.root.mkdir()
        patcher = mock.patch.object(REHEARSAL, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.tracked = mock.patch.object(
            REHEARSAL.subprocess, "run",
            return_value=subprocess.CompletedProcess([], 0, b"Cargo.toml\0target/tracked.json\0", b""),
        )
        self.tracked.start()
        self.addCleanup(self.tracked.stop)

    def invoke(self, path: Path | None) -> tuple[int, str, str, mock.Mock]:
        argv = ["release-rehearsal"]
        if path is not None:
            argv.extend(["--output", str(path)])
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(sys, "argv", argv), mock.patch.object(
            REHEARSAL, "build_rehearsal_receipt",
            return_value={"aggregate_status": "Incomplete"},
        ) as phases, contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = REHEARSAL.main()
        return result, stdout.getvalue(), stderr.getvalue(), phases

    def test_stdout_and_artifact_destinations_preserve_status_and_source(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("source sentinel", encoding="utf-8")
        result, stdout, stderr, phases = self.invoke(None)
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(stdout), {"aggregate_status": "Incomplete"})
        self.assertEqual(stderr, "")
        phases.assert_called_once()
        for output in (self.root / "target/receipts/new.json", self.sandbox / "external.json"):
            with self.subTest(output=output):
                result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual(result, 1)
                self.assertIn("Receipt written", stdout)
                self.assertEqual(stderr, "")
                self.assertEqual(json.loads(output.read_text()), {"aggregate_status": "Incomplete"})
                phases.assert_called_once()
        self.assertEqual(source.read_text(), "source sentinel")

    def test_source_tracked_and_invalid_destinations_reject_before_phases(self) -> None:
        directory = self.sandbox / "directory"
        directory.mkdir()
        parent_file = self.sandbox / "file"
        parent_file.write_text("sentinel")
        for output in (self.root / "Cargo.toml", self.root / "new-source.json",
                       self.root / "target/tracked.json", directory, parent_file / "receipt.json"):
            with self.subTest(output=output):
                result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual(result, 2)
                self.assertEqual(stdout, "")
                self.assertIn("instrumentation failed", stderr)
                phases.assert_not_called()
        self.assertEqual(parent_file.read_text(), "sentinel")

    def test_aliases_reject_before_phases(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("sentinel")
        link = self.sandbox / "alias"
        try:
            link.symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"directory symlinks unsupported: {error}")
        result, stdout, _, phases = self.invoke(link / "target/new.json")
        self.assertEqual((result, stdout), (2, ""))
        phases.assert_not_called()

    def test_hardlinked_leaf_rejects_before_phases(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("sentinel")
        link = self.sandbox / "alias"
        os.link(source, link)
        result, stdout, _, phases = self.invoke(link)
        self.assertEqual((result, stdout), (2, ""))
        phases.assert_not_called()
        self.assertEqual(source.read_text(), "sentinel")

    def test_unignored_artifact_and_git_failure_reject_before_phases(self) -> None:
        for failed_command in ("ls-files", "check-ignore"):
            with self.subTest(command=failed_command):
                def inspect(argv, **kwargs):
                    return subprocess.CompletedProcess(
                        argv, 1 if failed_command in argv else 0, b"", b"",
                    )
                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=inspect):
                    result, stdout, _, phases = self.invoke(self.root / "target/new.json")
                self.assertEqual((result, stdout), (2, ""))
                phases.assert_not_called()

    def test_existing_receipt_is_replaced_only_after_complete_write(self) -> None:
        output = self.sandbox / "receipt.json"
        output.write_text("old receipt")
        result, _, _, _ = self.invoke(output)
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(output.read_text()), {"aggregate_status": "Incomplete"})
        self.assertEqual(list(self.sandbox.glob(".rehearsal-*")), [])

    def test_failed_output_steps_preserve_existing_receipt(self) -> None:
        output = self.sandbox / "receipt.json"
        for operation in ("mkstemp", "fsync", "replace"):
            with self.subTest(operation=operation):
                output.write_text("old receipt")
                owner = REHEARSAL.tempfile if operation == "mkstemp" else REHEARSAL.os
                with mock.patch.object(owner, operation, side_effect=OSError("injected output failure")):
                    result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual((result, stdout), (2, ""))
                self.assertIn("instrumentation failed", stderr)
                phases.assert_called_once()
                self.assertEqual(output.read_text(), "old receipt")
                self.assertEqual(list(self.sandbox.glob(".rehearsal-*")), [])


class TestCandidateIdentity(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.candidate = Path(directory.name) / "candidate"
        self.candidate.write_bytes(b"identified candidate bytes")
        environment = mock.patch.dict(os.environ, {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        self.digest = REHEARSAL.compute_sha256(self.candidate)
        self.version = "0.2.0"
        self.projection = {
            "schema": "cargo-allow.release-identity.v1",
            "result": "validated",
            "version": "0.2.0",
            "tag": "v0.2.0",
            "tag_source": "derived",
            "channel": "stable",
            "rc_ordinal": None,
            "github_prerelease": False,
        }

    def invoke(self, *, result=None, error=None, candidate=None, digest=None):
        receipt = {}
        diagnostic = io.StringIO()
        if result is None:
            result = subprocess.CompletedProcess(
                [], 0, json.dumps(self.projection), "synthetic-private-child-output"
            )
        with (
            mock.patch.object(REHEARSAL, "_workspace_version", return_value=self.version),
            mock.patch.object(REHEARSAL.subprocess, "run", return_value=result,
                              side_effect=error) as run,
            contextlib.redirect_stderr(diagnostic),
        ):
            status = REHEARSAL.run_phase_release_identity(
                receipt, candidate_executable=candidate or self.candidate,
                candidate_sha256=digest or self.digest,
            )
        self.assertNotIn("synthetic-private", diagnostic.getvalue())
        return status, receipt, run, diagnostic.getvalue()

    def test_candidate_runs_from_verified_private_copy(self) -> None:
        observed = []

        def observe_copy(command, **kwargs):
            observed.append(Path(command[0]).read_bytes())
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")

        with mock.patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "synthetic-private-token"}):
            status, receipt, run, diagnostic = self.invoke(error=observe_copy)
        self.assertEqual(status, "Complete")
        self.assertEqual(receipt["release_identity"]["version"], "0.2.0")
        launched = Path(run.call_args.args[0][0])
        self.assertNotEqual(launched, self.candidate)
        self.assertEqual(observed, [self.candidate.read_bytes()])
        self.assertEqual(run.call_args.args[0][1:],
                         ["release-identity", "--version", "0.2.0"])
        self.assertFalse(launched.parent.exists())
        self.assertTrue(self.candidate.exists())
        self.assertNotIn("CARGO_REGISTRY_TOKEN", run.call_args.kwargs["env"])
        self.assertIn(self.digest, diagnostic)
        self.assertEqual(run.call_count, 1)

    def test_private_copy_preserves_executable_suffix_and_mode(self) -> None:
        self.candidate = self.candidate.with_suffix(".exe")
        self.candidate.write_bytes(b"identified candidate bytes")
        observed = []

        def observe_copy(command, **kwargs):
            copy = Path(command[0])
            observed.append((copy.suffix, copy.stat().st_mode))
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")

        status, _, _, _ = self.invoke(error=observe_copy)
        self.assertEqual(status, "Complete")
        self.assertEqual(observed[0][0], ".exe")
        if os.name != "nt":
            self.assertEqual(observed[0][1] & 0o777, 0o500)

    def test_private_copy_is_cleaned_after_child_failure_or_timeout(self) -> None:
        for timeout in (False, True):
            with self.subTest(timeout=timeout):
                observed = []

                def fail_child(command, **kwargs):
                    observed.append(Path(command[0]))
                    if timeout:
                        raise subprocess.TimeoutExpired(command, 300)
                    return subprocess.CompletedProcess([], 2, "", "")

                status, receipt, _, _ = self.invoke(error=fail_child)
                self.assertNotEqual(status, "Complete")
                self.assertNotIn("release_identity", receipt)
                self.assertFalse(observed[0].parent.exists())
                self.assertTrue(self.candidate.exists())

    def test_private_copy_failure_does_not_execute_or_expose_paths(self) -> None:
        copies = []

        def fail_copy(source, destination):
            copies.append(destination)
            raise OSError("synthetic-private-copy-path")

        with mock.patch.object(REHEARSAL.shutil, "copyfile", side_effect=fail_copy):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)
        run.assert_not_called()
        self.assertFalse(copies[0].parent.exists())

    def test_private_copy_digest_mismatch_does_not_execute(self) -> None:
        copies = []

        def invalid_copy(source, destination):
            copies.append(destination)
            destination.write_bytes(b"different copied bytes")

        with mock.patch.object(REHEARSAL.shutil, "copyfile", side_effect=invalid_copy):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("candidate_digest_mismatch", diagnostic)
        run.assert_not_called()
        self.assertFalse(copies[0].parent.exists())

    def test_private_copy_cleanup_failure_cannot_be_complete(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory

        @contextlib.contextmanager
        def cleanup_failure(**kwargs):
            with temporary_directory(**kwargs) as directory:
                yield directory
            raise OSError("synthetic-private-cleanup-path")

        with mock.patch.object(REHEARSAL.tempfile, "TemporaryDirectory",
                               side_effect=cleanup_failure):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)
        self.assertFalse(Path(run.call_args.args[0][0]).parent.exists())

    def test_missing_candidate_does_not_fall_back(self) -> None:
        status, receipt, run, diagnostic = self.invoke(
            candidate=self.candidate.with_name("missing")
        )
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        run.assert_not_called()
        self.assertIn("candidate_unavailable", diagnostic)

    def test_mismatched_digest_does_not_execute(self) -> None:
        status, receipt, run, diagnostic = self.invoke(digest="sha256:v1:" + "0" * 64)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        run.assert_not_called()
        self.assertIn("candidate_digest_mismatch", diagnostic)

    def test_invalid_selection_does_not_execute(self) -> None:
        for candidate, digest in (
            (Path("relative-candidate"), self.digest),
            (self.candidate, "sha256:v1:" + "G" * 64),
            (self.candidate, "synthetic-private-not-a-digest"),
        ):
            with self.subTest(candidate=candidate, digest=digest):
                status, receipt, run, diagnostic = self.invoke(candidate=candidate, digest=digest)
                self.assertEqual(status, "InstrumentFailure")
                self.assertNotIn("release_identity", receipt)
                run.assert_not_called()
                self.assertIn("candidate_selection_invalid", diagnostic)

    def test_unavailable_instrument_is_redacted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(error=OSError("synthetic-private"))
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)

    def test_changed_candidate_is_not_accepted(self) -> None:
        def change_candidate(*args, **kwargs):
            self.candidate.write_bytes(b"changed bytes")
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")
        status, receipt, _, diagnostic = self.invoke(error=change_candidate)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("candidate_changed", diagnostic)

    def test_failed_child_is_not_accepted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(
            result=subprocess.CompletedProcess([], 2, "synthetic-private", "synthetic-private")
        )
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("child_failed", diagnostic)

    def test_timeout_is_distinct_and_redacted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(
            error=subprocess.TimeoutExpired(["synthetic-private"], 300,
                                            output="synthetic-private")
        )
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("child_timeout", diagnostic)

    def test_malformed_identity_is_not_accepted(self) -> None:
        for output in ("invalid", "[]", "{}", json.dumps({**self.projection, "version": "9.9.9"}),
                       json.dumps({**self.projection, "github_prerelease": "false"})):
            with self.subTest(output=output):
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, output, "")
                )
                self.assertNotEqual(status, "Complete")
                self.assertNotIn("release_identity", receipt)

    def test_inconsistent_identity_is_not_accepted(self) -> None:
        for changes in (
            {"tag": ""}, {"tag": "v9.9.9"},
            {"tag_source": "observed"}, {"tag_source": "unknown"},
            {"rc_ordinal": 1}, {"github_prerelease": True},
            {"channel": "release_candidate", "rc_ordinal": 1, "github_prerelease": True},
        ):
            with self.subTest(changes=changes):
                projection = {**self.projection, **changes}
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, json.dumps(projection), "")
                )
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("release_identity", receipt)

    def test_rc_identity_matches_its_ordinal_and_prerelease_posture(self) -> None:
        self.version = "0.2.0-rc.2"
        self.projection.update(
            version=self.version, tag="v" + self.version, channel="release_candidate",
            rc_ordinal=2, github_prerelease=True,
        )
        status, receipt, _, _ = self.invoke()
        self.assertEqual(status, "Complete")
        self.assertEqual(receipt["release_identity"]["rc_ordinal"], 2)
        for changes in (
            {"rc_ordinal": None}, {"rc_ordinal": True}, {"rc_ordinal": 0},
            {"rc_ordinal": 1}, {"rc_ordinal": 4294967296},
            {"github_prerelease": False},
            {"channel": "stable", "rc_ordinal": None, "github_prerelease": False},
        ):
            with self.subTest(changes=changes):
                projection = {**self.projection, **changes}
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, json.dumps(projection), "")
                )
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("release_identity", receipt)

    def test_candidate_arguments_must_be_paired(self) -> None:
        for candidate, digest in ((self.candidate, None), (None, self.digest)):
            with self.subTest(candidate=candidate, digest=digest):
                with mock.patch.object(REHEARSAL, "resolve_commit") as resolve:
                    with self.assertRaises(ValueError):
                        REHEARSAL.build_rehearsal_receipt(
                            "HEAD", candidate_executable=candidate, candidate_sha256=digest
                        )
                    resolve.assert_not_called()

    def test_standalone_still_uses_deliberate_cargo_build(self) -> None:
        with (
            mock.patch.object(REHEARSAL, "_workspace_version", return_value="0.2.0"),
            mock.patch.object(REHEARSAL.subprocess, "run", return_value=
                              subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")) as run,
        ):
            self.assertEqual(REHEARSAL.run_phase_release_identity({}), "Complete")
        self.assertEqual(run.call_args.args[0][:2], ["cargo", "run"])


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

    def test_clean_non_ascii_linked_worktree_is_admitted(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-\u00e9-")
        self.addCleanup(directory.cleanup)
        worktree = Path(directory.name) / "checkout"
        self.git("worktree", "add", "--quiet", "--detach", str(worktree), self.head)
        self.addCleanup(self.git, "worktree", "remove", str(worktree))
        with mock.patch.object(REHEARSAL, "ROOT", worktree):
            receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertEqual(receipt["aggregate_status"], "Incomplete")
        for phase in self.phases:
            phase.assert_called_once()

    def test_repository_selection_environment_is_rejected_before_git(self) -> None:
        for name in (
            "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_NAMESPACE",
        ):
            with self.subTest(variable=name), mock.patch.dict(os.environ, {name: "fixture"}):
                with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                    with self.assertRaisesRegex(ValueError, "repository-selection environment"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    run.assert_not_called()
                self.require_no_phases()

    def test_admission_git_calls_use_supported_inspection_settings(self) -> None:
        with mock.patch.object(REHEARSAL.subprocess, "run", wraps=subprocess.run) as run:
            receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertTrue(run.call_args_list)
        for call in run.call_args_list:
            self.assertEqual(call.args[0][:2], ["git", "--no-replace-objects"])
            if "status" in call.args[0]:
                for setting in ("core.checkStat=default", "core.ignoreStat=false", "core.trustctime=true"):
                    self.assertIn(setting, call.args[0])

    def test_git_configuration_environment_is_rejected_before_git(self) -> None:
        for name in (
            "GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_PARAMETERS", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM",
            "GIT_CONFIG_NOSYSTEM",
        ):
            for value in ("", "fixture"):
                with self.subTest(variable=name, value=value), mock.patch.dict(os.environ, {name: value}):
                    with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                        with self.assertRaisesRegex(ValueError, "GIT_CONFIG"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
                        run.assert_not_called()
                    self.require_no_phases()

    def test_attribute_source_environment_is_rejected_before_git(self) -> None:
        for value in ("", "HEAD"):
            with self.subTest(value=value), mock.patch.dict(os.environ, {"GIT_ATTR_SOURCE": value}):
                with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                    with self.assertRaisesRegex(ValueError, "GIT_ATTR_SOURCE"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    run.assert_not_called()
                self.require_no_phases()

    def test_disabled_or_malformed_ctime_is_rejected_before_phases(self) -> None:
        for value, diagnostic in (
            ("false", "core.trustctime"),
            ("invalid-boolean", "not an exact repository commit"),
        ):
            with self.subTest(value=value):
                self.git("config", "core.trustctime", value)
                with self.assertRaisesRegex(ValueError, diagnostic):
                    REHEARSAL.build_rehearsal_receipt("HEAD")
                self.require_no_phases()
                self.assertEqual(self.git("config", "--get", "core.trustctime").strip(), value)

    def test_explicit_ctime_trust_preserves_characterization(self) -> None:
        self.git("config", "core.trustctime", "true")
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        for phase in self.phases:
            phase.assert_called_once()

    def test_ctime_inspection_failure_is_rejected_without_raw_output(self) -> None:
        run = subprocess.run

        def fail_config(command, **kwargs):
            if "config" in command:
                return subprocess.CompletedProcess(command, 128, b"secret-canary", b"secret-canary")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_config):
            with self.assertRaisesRegex(ValueError, "core.trustctime") as caught:
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertNotIn("secret-canary", str(caught.exception))
        self.require_no_phases()

    def test_disabled_ctime_after_phases_cannot_return_a_receipt(self) -> None:
        def change_config(receipt, *, candidate_executable=None, candidate_sha256=None):
            self.git("config", "core.trustctime", "false")
            return "Incomplete"

        self.phases[0].side_effect = change_config
        with self.assertRaisesRegex(ValueError, "core.trustctime"):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def require_transformation_rejected_before_status(self) -> None:
        run = subprocess.run

        def inspect_without_status(command, **kwargs):
            if "status" in command:
                return subprocess.CompletedProcess(command, 0, b"", b"")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=inspect_without_status) as calls:
            with self.assertRaisesRegex(ValueError, "content transformation"):
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertFalse(any("status" in call.args[0] for call in calls.call_args_list))
        self.require_no_phases()

    def test_content_transform_attributes_are_rejected_before_status(self) -> None:
        for attribute in (
            "filter", "filter=fixture", "filter=unspecified", "filter=unset", "-filter",
            "ident", "working-tree-encoding=UTF-8",
        ):
            with self.subTest(attribute=attribute):
                (self.root / ".git/info/attributes").write_text(
                    f"Cargo.lock {attribute}\n", encoding="utf-8",
                )
                self.require_transformation_rejected_before_status()

    def test_external_content_attributes_are_rejected_before_status(self) -> None:
        attributes = self.root / "target/fixture-attributes"
        attributes.parent.mkdir()
        attributes.write_text("Cargo.lock filter=fixture\n", encoding="utf-8")
        self.git("config", "core.attributesFile", str(attributes))
        self.require_transformation_rejected_before_status()

    def test_non_transform_attributes_preserve_characterization(self) -> None:
        (self.root / ".git/info/attributes").write_text(
            "Cargo.lock rehearsal-marker=present\n", encoding="utf-8",
        )
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        for phase in self.phases:
            phase.assert_called_once()

    def test_content_attribute_inspection_is_complete_and_redacted(self) -> None:
        run = subprocess.run
        for mutation in ("failed", "empty", "terminator", "extra", "path", "attribute"):
            with self.subTest(mutation=mutation):
                def corrupt_attributes(command, **kwargs):
                    result = run(command, **kwargs)
                    if "check-attr" not in command:
                        return result
                    output = result.stdout
                    if mutation == "empty":
                        output = b""
                    elif mutation == "terminator":
                        output = output.removesuffix(b"\0")
                    elif mutation == "extra":
                        output += b"extra\0"
                    elif mutation == "path":
                        output = output.replace(b".github/workflows/release.yml", b"other", 1)
                    elif mutation == "attribute":
                        output = output.replace(b"\0filter\0", b"\0other\0", 1)
                    return subprocess.CompletedProcess(
                        command, 128 if mutation == "failed" else 0, output, b"secret-canary",
                    )

                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=corrupt_attributes):
                    with self.assertRaisesRegex(ValueError, "content transformation") as caught:
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                self.assertNotIn("secret-canary", str(caught.exception))
                self.require_no_phases()

    def test_defined_attribute_inspection_is_validated(self) -> None:
        (self.root / ".git/info/attributes").write_text(
            "Cargo.lock rehearsal-marker=present\n", encoding="utf-8",
        )
        run = subprocess.run
        for mutation in ("failed", "terminator", "extra", "path", "duplicate"):
            with self.subTest(mutation=mutation):
                def corrupt_defined_attributes(command, **kwargs):
                    result = run(command, **kwargs)
                    if "check-attr" not in command or "--all" not in command:
                        return result
                    output = result.stdout
                    if mutation == "terminator":
                        output = output.removesuffix(b"\0")
                    elif mutation == "extra":
                        output += b"extra\0"
                    elif mutation == "path":
                        output = output.replace(b"Cargo.lock", b"other", 1)
                    elif mutation == "duplicate":
                        output += output
                    return subprocess.CompletedProcess(
                        command, 128 if mutation == "failed" else 0, output, b"secret-canary",
                    )

                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=corrupt_defined_attributes):
                    with self.assertRaisesRegex(ValueError, "content transformation") as caught:
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                self.assertNotIn("secret-canary", str(caught.exception))
                self.require_no_phases()

    def test_content_attributes_added_by_phase_prevent_receipt_return(self) -> None:
        def change_attributes(receipt, *, candidate_executable=None, candidate_sha256=None):
            (self.root / ".git/info/attributes").write_text("Cargo.lock filter\n", encoding="utf-8")
            return "Incomplete"

        self.phases[0].side_effect = change_attributes
        with self.assertRaisesRegex(ValueError, "content transformation"):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_different_discovered_root_is_rejected_before_phases(self) -> None:
        result = subprocess.CompletedProcess([], 0, os.fsencode(self.root.parent) + b"\n")
        with mock.patch.object(REHEARSAL, "resolve_commit", return_value=self.head):
            with mock.patch.object(REHEARSAL.subprocess, "run", return_value=result):
                with self.assertRaisesRegex(ValueError, "root does not match source root"):
                    REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_missing_or_relative_discovered_root_is_rejected(self) -> None:
        for code, output in ((1, b""), (0, b""), (0, b"relative-root\n")):
            with self.subTest(code=code, output=output):
                result = subprocess.CompletedProcess([], code, output)
                with mock.patch.object(REHEARSAL, "resolve_commit", return_value=self.head):
                    with mock.patch.object(REHEARSAL.subprocess, "run", return_value=result):
                        with self.assertRaisesRegex(ValueError, "checkout root"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
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
                    with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
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
        with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_external_ignore_cannot_hide_phase_input(self) -> None:
        self.add_untracked_source()
        exclude = self.root / "target/fixture-excludes"
        exclude.parent.mkdir()
        exclude.write_text(".changes/*.md\n", encoding="utf-8")
        self.git("config", "core.excludesFile", str(exclude))
        with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
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
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
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
                def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
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
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
            self.add_untracked_source()
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_staged_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
            (self.root / "Cargo.lock").write_text("staged by phase\n", encoding="utf-8")
            self.git("add", "Cargo.lock")
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_clean_commit_movement_after_phases_cannot_return_a_receipt(self) -> None:
        def change_head(receipt, *, candidate_executable=None, candidate_sha256=None):
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
