#!/usr/bin/env python3
"""Fail-closed tests for the release-rehearsal characterization."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

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
            self.assertIn("github-release", workflow["privileged_jobs"])
            self.assertEqual(
                workflow["top_level_permissions"],
                {"actions": "read", "contents": "write"},
            )

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
