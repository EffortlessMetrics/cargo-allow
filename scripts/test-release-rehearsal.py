#!/usr/bin/env python3
"""Fail-closed tests for the release-rehearsal characterization."""

from __future__ import annotations

import importlib.util
import contextlib
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


class TestCandidateIdentity(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.candidate = Path(directory.name) / "candidate"
        self.candidate.write_bytes(b"identified candidate bytes")
        self.digest = REHEARSAL.compute_sha256(self.candidate)
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
            mock.patch.object(REHEARSAL, "_workspace_version", return_value="0.2.0"),
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

    def test_candidate_runs_directly_with_observed_digest(self) -> None:
        with mock.patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "synthetic-private-token"}):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "Complete")
        self.assertEqual(receipt["release_identity"]["version"], "0.2.0")
        self.assertEqual(run.call_args.args[0],
                         [str(self.candidate), "release-identity", "--version", "0.2.0"])
        self.assertNotIn("CARGO_REGISTRY_TOKEN", run.call_args.kwargs["env"])
        self.assertIn(self.digest, diagnostic)
        self.assertEqual(run.call_count, 1)

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


if __name__ == "__main__":
    unittest.main()
